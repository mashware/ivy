use super::{Shared, Status, error, verify, worker::Entry};
use panel_control::session::{Client, Identity, Peer, Request};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

pub(super) fn apply(entry: &mut Entry, shared: &Shared, peer: Peer, request: Request) -> Value {
    if request.validate().is_err() {
        return error("invalid_request");
    }
    if entry.frozen || shared.frozen.load(Ordering::Acquire) {
        return error("closing");
    }
    let Ok(root) = verify::root(&shared.scope, &peer, &entry.programs) else {
        return error("unauthorized");
    };
    if !shared.enabled.load(Ordering::Acquire)
        || shared.epoch.load(Ordering::Acquire) != entry.epoch
    {
        return error("disabled");
    }
    match request {
        Request::Observe {
            client,
            session_id,
            cwd,
            event,
            source: _,
        } => {
            if client != Client::Codex || root.client != client {
                return error("unauthorized");
            }
            if event == "SessionEnd" {
                return json!({"ok":true});
            }
            if event != "SessionStart" {
                return error("invalid_request");
            }
            let Ok(identity) = Identity::new(client, session_id) else {
                return error("invalid_request");
            };
            if !commit(
                shared,
                entry.epoch,
                Some(identity),
                Some(PathBuf::from(cwd)),
                true,
                false,
            ) {
                return error("disabled");
            }
            entry.root = Some(root);
            entry.generation = None;
            json!({"ok":true})
        }
        Request::Attach { client } => {
            if client == Client::Codex || client != root.client {
                return error("unauthorized");
            }
            let Ok(generation) = token() else {
                return error("unavailable");
            };
            if !commit(shared, entry.epoch, None, None, false, false) {
                return error("disabled");
            }
            entry.root = Some(root);
            entry.generation = Some(generation.clone());
            entry.sequence = 0;
            // Reattaching cannot acknowledge a read for a challenge the new reporter never saw.
            json!({"ok":true,"generation":generation})
        }
        Request::Probe { generation } => {
            if !same_generation(entry, &generation, &root) {
                return error("stale_generation");
            }
            json!({"ok":true,"challenge":entry.challenge})
        }
        Request::Update {
            generation,
            sequence,
            session_id,
            cwd,
            challenge,
        } => {
            if !same_generation(entry, &generation, &root) {
                return error("stale_generation");
            }
            if sequence <= entry.sequence {
                return error("stale_sequence");
            }
            let identity = match session_id {
                Some(session_id) => match Identity::new(root.client, session_id) {
                    Ok(identity) => Some(identity),
                    Err(_) => return error("invalid_request"),
                },
                None => None,
            };
            let cwd = match cwd {
                Some(cwd) => Some(PathBuf::from(cwd)),
                None if root.client == Client::Claude && identity.is_some() => {
                    match root.working_directory(&shared.scope) {
                        Ok(directory) => Some(directory),
                        Err(_) => {
                            entry.revoke(shared);
                            return error("unavailable_directory");
                        }
                    }
                }
                None => None,
            };
            let acknowledged = entry.challenge.is_some() && challenge == entry.challenge;
            if !commit(shared, entry.epoch, identity, cwd, true, acknowledged) {
                return error("disabled");
            }
            entry.sequence = sequence;
            json!({"ok":true})
        }
    }
}

fn same_generation(entry: &Entry, generation: &str, root: &verify::Root) -> bool {
    entry.generation.as_deref() == Some(generation)
        && entry.root.as_ref().is_some_and(|previous| {
            previous.client == root.client
                && previous.process.pid == root.process.pid
                && previous.process.birth == root.process.birth
        })
}

fn commit(
    shared: &Shared,
    epoch: u64,
    identity: Option<Identity>,
    cwd: Option<PathBuf>,
    sampled: bool,
    acknowledged: bool,
) -> bool {
    let mut snapshot = shared
        .published
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if !shared.enabled.load(Ordering::Acquire)
        || shared.frozen.load(Ordering::Acquire)
        || shared.epoch.load(Ordering::Acquire) != epoch
        || !shared.scope.is_open()
    {
        return false;
    }
    if acknowledged {
        shared.close_acknowledged.store(true, Ordering::Release);
    }
    let changed = snapshot.identity != identity
        || snapshot.cwd != cwd
        || snapshot.status != Status::Registered
        || snapshot.sampled != sampled;
    if changed {
        snapshot.identity = identity;
        snapshot.cwd = cwd;
        snapshot.status = Status::Registered;
        snapshot.sampled = sampled;
        snapshot.revision = snapshot.revision.wrapping_add(1);
    }
    drop(snapshot);
    if changed || acknowledged {
        shared.changed.notify_all();
    }
    if changed {
        (shared.wake)();
    }
    true
}

pub(super) fn token() -> Result<String, getrandom::Error> {
    let mut random = [0; 16];
    getrandom::fill(&mut random)?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}
