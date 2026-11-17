//! Producer-side interest checks. Routing remains authoritative at delivery time.
use super::{BroadcastTopics, OutboundWebSocketMessages};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::{UnboundedSender, error::SendError};

#[derive(Default)]
pub(crate) struct SubscriptionInterest {
    any: AtomicBool,
    // None is an explicit global route, not a wildcard for entity subscriptions.
    counts: RwLock<HashMap<String, HashMap<Option<i64>, usize>>>,
}

impl SubscriptionInterest {
    // Call only for actual membership changes, under ClientsState's write lock.
    pub(crate) fn add(&self, topic: &str, id: Option<i64>) {
        let mut counts = self.counts.write();
        *counts
            .entry(topic.to_owned())
            .or_default()
            .entry(id)
            .or_default() += 1;
        self.any.store(true, Ordering::Release);
    }

    pub(crate) fn remove(&self, topic: &str, id: Option<i64>) {
        let mut counts = self.counts.write();
        if let Some(ids) = counts.get_mut(topic) {
            if let Some(count) = ids.get_mut(&id) {
                *count -= 1;
                if *count == 0 {
                    ids.remove(&id);
                }
            }
            if ids.is_empty() {
                counts.remove(topic);
            }
        }
        self.any.store(!counts.is_empty(), Ordering::Release);
    }

    pub(crate) fn has_any(&self) -> bool {
        self.any.load(Ordering::Acquire)
    }

    pub(crate) fn matches(&self, topics: &BroadcastTopics) -> bool {
        if !self.has_any() {
            return false;
        }
        let counts = self.counts.read();
        topics.iter().any(|(topic, id)| {
            counts
                .get(topic.as_ref())
                .is_some_and(|ids| ids.contains_key(id))
        })
    }
}

#[derive(Clone)]
pub(crate) struct OutboundSender {
    inner: UnboundedSender<OutboundWebSocketMessages>,
    interest: Arc<SubscriptionInterest>,
}

impl OutboundSender {
    pub(crate) fn new(
        inner: UnboundedSender<OutboundWebSocketMessages>,
        interest: Arc<SubscriptionInterest>,
    ) -> Self {
        Self { inner, interest }
    }

    pub(crate) fn is_interested(&self, topics: &BroadcastTopics) -> bool {
        self.interest.matches(topics)
    }

    pub(crate) fn has_subscribers(&self) -> bool {
        self.interest.has_any()
    }

    pub(crate) fn send(
        &self,
        message: OutboundWebSocketMessages,
    ) -> Result<(), Box<SendError<OutboundWebSocketMessages>>> {
        // Avoid even dynamic topic-name allocation in the no-subscriber case.
        if self.interest.has_any()
            && message
                .topics()
                .is_some_and(|topics| self.is_interested(&topics))
        {
            self.inner.send(message).map_err(Box::new)
        } else {
            Ok(())
        }
    }

    /// Only notification preparation belongs in this closure, never state writes.
    /// A subscribe concurrent with this check may start with the next update;
    /// subsequent delivery still rechecks the client's current subscriptions.
    pub(crate) fn send_if_interested(
        &self,
        topics: BroadcastTopics,
        make_message: impl FnOnce() -> OutboundWebSocketMessages,
    ) -> Result<(), Box<SendError<OutboundWebSocketMessages>>> {
        if self.is_interested(&topics) {
            self.inner.send(make_message()).map_err(Box::new)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientsState, WebsocketEncoding};

    async fn client(
        clients: &ClientsState,
        id: &str,
    ) -> kanal::AsyncReceiver<OutboundWebSocketMessages> {
        let (tx, rx) = kanal::bounded_async(20);
        clients
            .add_client(id.to_owned(), tx, WebsocketEncoding::Json)
            .await;
        rx
    }

    fn experience(id: i64) -> OutboundWebSocketMessages {
        OutboundWebSocketMessages::Experience {
            experience: 100,
            level: 1,
            rank: 2,
            skill_name: "Mining".into(),
            user_id: id,
        }
    }

    #[tokio::test]
    async fn membership_counts_survive_duplicates_missing_clients_and_disconnects() {
        let clients = ClientsState::new();
        let _a = client(&clients, "a").await;
        let _b = client(&clients, "b").await;
        let topic = "experience".to_owned();
        let routes = experience(7).topics().unwrap();
        let a = "a".to_owned();
        let b = "b".to_owned();
        let missing = "missing".to_owned();
        clients.add_topic_to_client(&missing, &topic, Some(7)).await;
        assert!(!clients.interest.has_any());
        tokio::join!(
            clients.add_topic_to_client(&a, &topic, Some(7)),
            clients.add_topic_to_client(&a, &topic, Some(7)),
        );
        clients.add_topic_to_client(&b, &topic, Some(7)).await;
        clients
            .remove_topic_from_client(&missing, &topic, Some(7))
            .await;
        clients.remove_topic_from_client(&a, &topic, Some(7)).await;
        clients.remove_topic_from_client(&a, &topic, Some(7)).await;
        assert!(clients.interest.matches(&routes), "b still subscribes");
        clients
            .add_topic_to_client(&a, "insert_sell_order", None)
            .await;
        clients.remove_client(&a).await;
        clients.remove_client(&a).await;
        assert!(clients.interest.matches(&routes));
        clients.remove_client(&b).await;
        assert!(!clients.interest.has_any());
        assert!(clients.interest.counts.read().is_empty());
    }

    #[tokio::test]
    async fn replacing_a_client_cleans_up_scoped_and_global_interest() {
        let clients = ClientsState::new();
        let _old = client(&clients, "a").await;
        clients
            .add_topic_to_client(&"a".into(), "experience", Some(7))
            .await;
        clients
            .add_topic_to_client(&"a".into(), "insert_sell_order", None)
            .await;
        let _replacement = client(&clients, "a").await;
        assert!(!clients.interest.has_any());
        clients
            .add_topic_to_client(&"a".into(), "experience", Some(8))
            .await;
        assert!(!clients.interest.matches(&experience(7).topics().unwrap()));
        assert!(clients.interest.matches(&experience(8).topics().unwrap()));
    }

    #[tokio::test]
    async fn lazy_send_skips_preparation_and_preserves_alias_delivery_and_unsubscribe() {
        let clients = ClientsState::new();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = OutboundSender::new(tx, clients.interest.clone());
        let client_rx = client(&clients, "a").await;
        let id = "a".to_owned();
        let routes = || experience(7).topics().unwrap();
        sender
            .send_if_interested(routes(), || panic!("no interested client"))
            .unwrap();
        clients
            .add_topic_to_client(&id, "experience", Some(8))
            .await;
        // An unscoped subscription is not a wildcard for per-player messages.
        clients
            .add_topic_to_client(&id, "experience", None)
            .await;
        sender
            .send_if_interested(routes(), || panic!("wrong player or scope"))
            .unwrap();
        assert!(rx.try_recv().is_err());
        for topic in ["experience:Mining", "experience"] {
            clients
                .add_topic_to_client(&id, topic, Some(7))
                .await;
        }
        sender
            .send_if_interested(routes(), || experience(7))
            .unwrap();
        let message = rx.try_recv().unwrap();
        clients
            .broadcast_topics(message.topics().unwrap(), &message)
            .await;
        // Preserve the existing duplicate delivery for overlapping aliases.
        assert!(client_rx.try_recv().unwrap().is_some());
        assert!(client_rx.try_recv().unwrap().is_some());
        assert!(client_rx.try_recv().unwrap().is_none());
        // Queued messages still use current routing when a client unsubscribes.
        sender.send(experience(7)).unwrap();
        for topic in ["experience:Mining", "experience"] {
            clients
                .remove_topic_from_client(&id, &topic.into(), Some(7))
                .await;
        }
        let message = rx.try_recv().unwrap();
        clients
            .broadcast_topics(message.topics().unwrap(), &message)
            .await;
        assert!(client_rx.try_recv().unwrap().is_none());
        sender.send(experience(7)).unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn global_routes_and_inventory_owner_aliases_are_indexed_exactly() {
        let clients = ClientsState::new();
        let _rx = client(&clients, "a").await;
        let id = "a".to_owned();
        clients
            .add_topic_to_client(&id, "insert_sell_order", None)
            .await;
        assert!(
            clients
                .interest
                .matches(&BroadcastTopics::new([("insert_sell_order".into(), None)]))
        );
        assert!(!clients.interest.matches(&BroadcastTopics::new([(
            "insert_sell_order".into(),
            Some(1)
        )])));
        clients
            .remove_topic_from_client(&id, &"insert_sell_order".into(), None)
            .await;
        // Repeated removal with a scoped subscription for the same name must not panic.
        clients
            .add_topic_to_client(&id, "insert_sell_order", Some(1))
            .await;
        clients
            .remove_topic_from_client(&id, &"insert_sell_order".into(), None)
            .await;
        for topic in [
            "inventory_insert",
            "inventory_insert_owner",
            "inventory_insert_player_owner",
        ] {
            clients
                .add_topic_to_client(&id, topic, Some(42))
                .await;
            assert!(
                clients
                    .interest
                    .matches(&BroadcastTopics::inventory_insert(42, 42, 42))
            );
            assert!(
                !clients
                    .interest
                    .matches(&BroadcastTopics::inventory_insert(1, 2, 3))
            );
            clients
                .remove_topic_from_client(&id, &topic.into(), Some(42))
                .await;
        }
        clients.remove_client(&id).await;
        assert!(!clients.interest.has_any());
    }
}
