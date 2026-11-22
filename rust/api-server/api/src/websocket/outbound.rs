use super::OutboundWebSocketMessages;
use crate::WebsocketEncoding;
use futures::{Sink, SinkExt};
use sockudo_ws::Message;
use tokio::time::{Duration, error::Elapsed, timeout};

const MESSAGES_SENT: &str = "websocket_outbound_messages_total";
const PAYLOAD_BYTES_SENT: &str = "websocket_outbound_payload_bytes_total";

pub(crate) fn describe_metrics() {
    metrics::describe_counter!(
        MESSAGES_SENT,
        "Application messages successfully written to client WebSockets."
    );
    metrics::describe_counter!(
        PAYLOAD_BYTES_SENT,
        "Serialized payload bytes successfully written to client WebSockets, before compression and excluding transport overhead."
    );
}

pub(crate) async fn send<S: Sink<Message> + Unpin>(
    sender: &mut S,
    message: &OutboundWebSocketMessages,
    encoding: WebsocketEncoding,
    write_timeout: Duration,
) -> Result<Result<(), S::Error>, Elapsed> {
    let (frame, payload_bytes) = match encoding {
        WebsocketEncoding::Json => {
            let payload = serde_json::to_string(message).unwrap();
            let bytes = payload.len();
            (Message::text(payload), bytes)
        }
        WebsocketEncoding::MessagePack => {
            let payload = axum_codec::Codec(message).to_msgpack().unwrap();
            let bytes = payload.len();
            (Message::Binary(payload.into()), bytes)
        }
    };

    let result = timeout(write_timeout, sender.send(frame)).await;
    if let Ok(Ok(())) = &result {
        let message_type = message.message_type();
        let encoding = encoding.as_str();
        metrics::counter!(MESSAGES_SENT, "message_type" => message_type, "encoding" => encoding)
            .increment(1);
        metrics::counter!(PAYLOAD_BYTES_SENT, "message_type" => message_type, "encoding" => encoding)
            .increment(payload_bytes as u64);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, channel::mpsc, sink};
    use metrics_exporter_prometheus::PrometheusBuilder;

    #[tokio::test]
    async fn counts_serialized_bytes_per_type_encoding_and_recipient() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let _guard = metrics::set_default_local_recorder(&recorder);
        describe_metrics();
        let (mut first, mut first_rx) = mpsc::unbounded();
        let (mut second, mut second_rx) = mpsc::unbounded();
        let message = OutboundWebSocketMessages::Message("café".into());
        let write_timeout = Duration::from_secs(1);

        for sender in [&mut first, &mut second] {
            send(sender, &message, WebsocketEncoding::Json, write_timeout)
                .await
                .unwrap()
                .unwrap();
        }
        let expected_json = r#"{"t":"Message","c":"café"}"#;
        for receiver in [&mut first_rx, &mut second_rx] {
            let Some(Message::Text(text)) = receiver.next().await else {
                panic!("expected a JSON text frame");
            };
            assert_eq!(text.as_ref(), expected_json.as_bytes());
        }

        send(
            &mut first,
            &message,
            WebsocketEncoding::MessagePack,
            write_timeout,
        )
        .await
        .unwrap()
        .unwrap();
        let Some(Message::Binary(binary)) = first_rx.next().await else {
            panic!("expected a MessagePack binary frame");
        };
        assert_eq!(
            binary.as_ref(),
            axum_codec::Codec(&message).to_msgpack().unwrap().as_slice()
        );

        send(
            &mut first,
            &OutboundWebSocketMessages::SubscribedTopics(vec!["topic.123".into()]),
            WebsocketEncoding::Json,
            write_timeout,
        )
        .await
        .unwrap()
        .unwrap();
        let Some(Message::Text(topics)) = first_rx.next().await else {
            panic!("expected a subscription reply");
        };

        let rendered = recorder.handle().render();
        for (kind, encoding, count, bytes) in [
            ("Message", "json", 2, expected_json.len() * 2),
            ("Message", "messagepack", 1, binary.len()),
            ("SubscribedTopics", "json", 1, topics.len()),
        ] {
            assert_sample(&rendered, MESSAGES_SENT, kind, encoding, count);
            assert_sample(&rendered, PAYLOAD_BYTES_SENT, kind, encoding, bytes);
        }
    }

    fn assert_sample(rendered: &str, name: &str, kind: &str, encoding: &str, value: usize) {
        assert!(
            rendered.lines().any(|line| {
                line.starts_with(&format!("{name}{{"))
                    && line.contains(&format!(r#"message_type="{kind}""#))
                    && line.contains(&format!(r#"encoding="{encoding}""#))
                    && line.ends_with(&format!("}} {value}"))
            }),
            "missing {name} for {kind}/{encoding} with value {value}:\n{rendered}"
        );
    }

    #[tokio::test]
    async fn does_not_count_failed_or_timed_out_writes() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let _guard = metrics::set_default_local_recorder(&recorder);
        let message = OutboundWebSocketMessages::Message("not sent".into());
        let mut failing = sink::unfold((), |(), _: Message| {
            std::future::ready(Err::<(), _>("closed"))
        });
        let mut stalled = sink::unfold((), |(), _: Message| {
            std::future::pending::<Result<(), ()>>()
        });

        assert!(matches!(
            send(
                &mut failing,
                &message,
                WebsocketEncoding::Json,
                Duration::from_secs(1)
            )
            .await,
            Ok(Err("closed"))
        ));
        assert!(
            send(
                &mut stalled,
                &message,
                WebsocketEncoding::Json,
                Duration::ZERO
            )
            .await
            .is_err()
        );
        let rendered = recorder.handle().render();
        assert!(!rendered.contains(MESSAGES_SENT));
        assert!(!rendered.contains(PAYLOAD_BYTES_SENT));
    }
}
