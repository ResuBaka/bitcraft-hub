use super::{DbConnection, MobileEntityState, PlayerState};
use crate::world_map::RegionFilter;
use entity::shared::Region;
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StreamKind {
    AtlasPlayers,
}

// HTTP upgrade uses the same path as the other browser WebSocket streams.
#[derive(Clone)]
struct Frame {
    revision: u64,
    full: Arc<str>,
    delta: Arc<str>,
    players: Arc<Players>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position {
    timestamp: u64,
    x: i32,
    z: i32,
}

#[derive(Default)]
struct RegionalPlayers {
    ready: bool,
    online: HashSet<u64>,
    positions: HashMap<u64, Position>,
}

pub(crate) struct AtlasPlayers {
    regions: RwLock<HashMap<Region, RegionalPlayers>>,
    frames: watch::Sender<Frame>,
}

impl Default for AtlasPlayers {
    fn default() -> Self {
        Self {
            regions: RwLock::default(),
            frames: watch::Sender::new(Frame {
                revision: 0,
                full: r#"{"type":"snapshot","players":[]}"#.into(),
                delta: "".into(),
                players: Arc::default(),
            }),
        }
    }
}

type Players = HashMap<u64, (Region, i32, i32)>;

impl AtlasPlayers {
    fn update(&self, region: Region, f: impl FnOnce(&mut RegionalPlayers)) {
        if let Some(rows) = self.regions.write().get_mut(&region) {
            f(rows);
        }
    }

    pub(crate) fn invalidate(&self, region: Region) {
        self.regions.write().remove(&region);
    }

    pub(crate) fn ready(&self, region: Region) {
        self.update(region, |r| r.ready = true);
    }

    fn online_positions(&self) -> Players {
        let mut latest = HashMap::<u64, (Region, Position)>::new();
        for (&region, rows) in self.regions.read().iter().filter(|(_, rows)| rows.ready) {
            for id in &rows.online {
                if let Some(&p) = rows.positions.get(id) {
                    let current = latest.entry(*id).or_insert((region, p));
                    let (old_region, old) = *current;
                    if (p.timestamp, p.x, p.z, region) > (old.timestamp, old.x, old.z, old_region) {
                        *current = (region, p);
                    }
                }
            }
        }
        latest
            .into_iter()
            .map(|(id, (region, p))| (id, (region, p.x, p.z)))
            .collect()
    }

    pub(crate) fn start(self: Arc<Self>, stop: CancellationToken) {
        tokio::spawn(async move {
            let mut previous = Players::new();
            let mut revision = 0;
            let mut tick = tokio::time::interval(Duration::from_millis(500));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    _ = tick.tick() => {
                        if self.frames.receiver_count() == 0 {
                            if !previous.is_empty() {
                                revision += 1;
                                let departed = std::mem::take(&mut previous);
                                self.frames.send_replace(make_frame(revision, &departed, &previous));
                            }
                            continue;
                        }
                        let next = self.online_positions();
                        if next != previous {
                            revision += 1;
                            self.frames.send_replace(make_frame(revision, &previous, &next));
                            previous = next;
                        }
                    }
                }
            }
        });
    }

    pub(crate) async fn serve(
        &self,
        socket: crate::WebSocket,
        stop: CancellationToken,
        filter: RegionFilter,
    ) {
        let (mut writer, mut reader) = StreamExt::split(socket);
        let mut frames = self.frames.subscribe();
        let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
        heartbeat.tick().await;
        let mut last = None;
        let mut filtered = None;
        loop {
            let message = if last.is_none() {
                let frame = frames.borrow_and_update();
                last = Some(frame.revision);
                let text = if filter.ids().is_some() {
                    filtered_frame(&frame.players, &filter, &mut filtered)
                        .expect("initial snapshot")
                } else {
                    frame.full.to_string()
                };
                crate::Message::Text(text.into())
            } else {
                tokio::select! {
                    _ = stop.cancelled() => {
                        let _ = tokio::time::timeout(Duration::from_secs(1), writer.send(crate::Message::Close(None))).await;
                        break;
                    }
                    incoming = reader.next() => match incoming {
                        Some(Ok(crate::Message::Close(_)) | Err(_)) | None => break,
                        _ => continue,
                    },
                    changed = frames.changed() => {
                        if changed.is_err() { break; }
                        let frame = frames.borrow_and_update();
                        let text = if filter.ids().is_some() {
                            filtered_frame(&frame.players, &filter, &mut filtered)
                        } else {
                            Some(if last == frame.revision.checked_sub(1) { &frame.delta } else { &frame.full }.to_string())
                        };
                        last = Some(frame.revision);
                        let Some(text) = text else { continue; };
                        crate::Message::Text(text.into())
                    }
                    _ = heartbeat.tick() => crate::Message::Ping(Vec::new().into()),
                }
            };
            if !matches!(
                tokio::time::timeout(crate::WEBSOCKET_CLIENT_WRITE_TIMEOUT, writer.send(message))
                    .await,
                Ok(Ok(()))
            ) {
                break;
            }
        }
    }
}

#[derive(Serialize)]
struct Marker(String, i32, i32);

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Update {
    Snapshot {
        players: Vec<Marker>,
    },
    Delta {
        upsert: Vec<Marker>,
        remove: Vec<String>,
    },
}

fn snapshot(players: &Players) -> Update {
    Update::Snapshot {
        players: players
            .iter()
            .map(|(id, &(_, x, z))| Marker(id.to_string(), x, z))
            .collect(),
    }
}

fn delta(previous: &Players, next: &Players) -> Update {
    Update::Delta {
        upsert: next
            .iter()
            .filter(|(id, point)| previous.get(*id) != Some(*point))
            .map(|(id, &(_, x, z))| Marker(id.to_string(), x, z))
            .collect(),
        remove: previous
            .keys()
            .filter(|id| !next.contains_key(id))
            .map(u64::to_string)
            .collect(),
    }
}

fn filtered_frame(
    players: &Players,
    filter: &RegionFilter,
    previous: &mut Option<Players>,
) -> Option<String> {
    let current: Players = players
        .iter()
        .filter(|(_, (region, _, _))| filter.includes(*region))
        .map(|(&id, &point)| (id, point))
        .collect();
    let update = match previous.as_ref() {
        Some(old) if *old == current => return None,
        Some(old) => delta(old, &current),
        None => snapshot(&current),
    };
    *previous = Some(current);
    Some(serde_json::to_string(&update).expect("serializable filtered atlas update"))
}

fn make_frame(revision: u64, previous: &Players, next: &Players) -> Frame {
    Frame {
        revision,
        players: Arc::new(next.clone()),
        full: serde_json::to_string(&snapshot(next))
            .expect("serializable atlas snapshot")
            .into(),
        delta: serde_json::to_string(&delta(previous, next))
            .expect("serializable atlas delta")
            .into(),
    }
}

impl RegionalPlayers {
    fn presence(&mut self, row: &PlayerState) {
        if row.signed_in {
            self.online.insert(row.entity_id);
        } else {
            self.online.remove(&row.entity_id);
        }
    }

    fn position(&mut self, row: &MobileEntityState) {
        if row.dimension == 1 {
            self.positions.insert(
                row.entity_id,
                Position {
                    timestamp: row.timestamp,
                    x: row.location_x,
                    z: row.location_z,
                },
            );
        } else {
            self.positions.remove(&row.entity_id);
        }
    }
}

pub(super) fn listen(ctx: &DbConnection, region: Region, atlas: Arc<AtlasPlayers>) {
    use game_module::module_bindings::{MobileEntityStateTableAccess, PlayerStateTableAccess};
    atlas
        .regions
        .write()
        .insert(region, RegionalPlayers::default());
    let presence = ctx.db.player_state();
    let target = atlas.clone();
    presence.on_initial_rows(move |_, rows| {
        target.update(region, |r| {
            for row in rows.iter() {
                r.presence(row);
            }
        })
    });
    let target = atlas.clone();
    presence.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target.update(region, |r| r.presence(row));
        }
    });
    let target = atlas.clone();
    presence.on_update(move |_, _, row| target.update(region, |r| r.presence(row)));
    let target = atlas.clone();
    presence.on_delete(move |_, row| {
        target.update(region, |r| {
            r.online.remove(&row.entity_id);
        })
    });

    let position = ctx.db.mobile_entity_state();
    let target = atlas.clone();
    position.on_initial_rows(move |_, rows| {
        target.update(region, |r| {
            for row in rows.iter() {
                r.position(row);
            }
        })
    });
    let target = atlas.clone();
    position.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target.update(region, |r| r.position(row));
        }
    });
    let target = atlas.clone();
    position.on_update(move |_, _, row| target.update(region, |r| r.position(row)));
    position.on_delete(move |_, row| {
        atlas.update(region, |r| {
            r.positions.remove(&row.entity_id);
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn read_frame(client: &mut tokio::net::TcpStream) -> serde_json::Value {
        assert_eq!(client.read_u8().await.unwrap(), 0x81);
        let length = client.read_u8().await.unwrap();
        let mut body = vec![0; usize::from(length)];
        client.read_exact(&mut body).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn atlas_players_websocket_sends_snapshot_movement_and_removal() {
        tokio::time::timeout(Duration::from_secs(5), async {
            let (tx, _) = tokio::sync::mpsc::unbounded_channel();
            let state = crate::AppState::new(sea_orm::DatabaseConnection::default(), tx, Default::default());
            let atlas = state.atlas_players.clone();
            let id = u64::MAX;
            atlas.regions.write().insert(7, RegionalPlayers::default());
            atlas.update(7, |r| {
                r.online.insert(id);
                r.positions.insert(id, Position { timestamp: 10, x: 100, z: 50 });
                r.positions.insert(2, Position { timestamp: 10, x: 100, z: 50 });
            });
            atlas.regions.write().insert(8, RegionalPlayers::default());
            atlas.update(8, |r| {
                r.online.insert(2);
                r.positions.insert(2, Position { timestamp: 10, x: 300, z: 50 });
            });
            atlas.ready(8);
            assert!(!atlas.online_positions().contains_key(&id));
            atlas.ready(7);
            let initial = atlas.online_positions();
            assert_eq!(initial, HashMap::from([(id, (7, 100, 50)), (2, (8, 300, 50))]));
            atlas.frames.send_replace(make_frame(1, &Players::new(), &initial));
            let shutdown = state.shutdown.clone();
            let app = axum::Router::new().route("/websocket", axum::routing::get(crate::websocket_handler)).with_state(state);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let stop = shutdown.stop.clone();
            let server = tokio::spawn(async move {
                axum::serve(listener, app).with_graceful_shutdown(stop.cancelled_owned()).await
            });
            let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
            client.write_all(b"GET /websocket?stream=atlasPlayers&regions=7 HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(client.read_u8().await.unwrap());
            }
            assert!(String::from_utf8_lossy(&header).contains("101 Switching Protocols"));
            assert_eq!(read_frame(&mut client).await, serde_json::json!({"type":"snapshot","players":[[id.to_string(),100,50]]}));

            atlas.update(7, |r| { r.positions.get_mut(&id).unwrap().x = 125; });
            let moved = atlas.online_positions();
            atlas.frames.send_replace(make_frame(2, &initial, &moved));
            assert_eq!(read_frame(&mut client).await, serde_json::json!({"type":"delta","upsert":[[id.to_string(),125,50]],"remove":[]}));

            atlas.update(8, |r| {
                r.online.insert(id);
                r.positions.insert(id, Position { timestamp: 11, x: 450, z: 50 });
            });
            atlas.frames.send_replace(make_frame(3, &moved, &atlas.online_positions()));
            assert_eq!(read_frame(&mut client).await, serde_json::json!({"type":"delta","upsert":[],"remove":[id.to_string()]}));

            shutdown.drain().await;
            server.await.unwrap().unwrap();
        }).await.unwrap();
    }
}
