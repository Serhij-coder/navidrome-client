//! Test-only real browser backup persistence entry, not an application export.
use feishin_domain::{PlaylistId, Revision, Scoped, ServerId};
use feishin_storage::{
    BrowserStore,
    playlist_backups::{Backup, Repository, Snapshot},
};
use wasm_bindgen::prelude::*;
fn backup(server: &str) -> Backup {
    Backup {
        server_id: server.into(),
        playlist_id: "p".into(),
        name: "Canonical".into(),
        song_ids: vec!["same".into(), "other".into(), "same".into()],
        captured_at_unix_ms: 1234567890123,
    }
}
#[wasm_bindgen]
pub fn playlist_backup_probe(action: &str) -> Result<bool, JsValue> {
    run(action).map_err(|e| JsValue::from_str(&e.to_string()))
}
fn run(action: &str) -> Result<bool, feishin_storage::playlist_backups::Error> {
    let mut repo = Repository::new(BrowserStore::open_playlist_backups()?);
    match action {
        "defaults" => Ok(repo.load()? == Snapshot::default()),
        "save-scopes" => {
            repo.save(&backup("a"), Revision(0))?;
            repo.save(&backup("b"), Revision(1))?;
            let s = repo.load()?;
            Ok(s.revision == Revision(2)
                && s.backups.len() == 2
                && s.backups[0].song_ids.len() == 3)
        }
        "reload" => {
            let s = repo.load()?;
            Ok(s.revision == Revision(2)
                && s.get(&backup("a").identity()) == Some(&backup("a"))
                && s.get(&backup("b").identity()) == Some(&backup("b")))
        }
        "try-save" => {
            let s = repo.load()?;
            repo.save(&backup("a"), s.revision)?;
            Ok(true)
        }
        "remove-a" => {
            let s = repo.load()?;
            repo.remove(
                &Scoped {
                    server: ServerId::new("a"),
                    id: PlaylistId::new("p"),
                },
                s.revision,
            )?;
            let s = repo.load()?;
            Ok(s.backups.len() == 1 && s.backups[0] == backup("b"))
        }
        _ => Err(feishin_storage::playlist_backups::Error::InvalidDocument),
    }
}

/// Poll the local future directly: this adapter performs no pending background work.
#[wasm_bindgen]
pub fn playlist_backup_port_probe(action: &str) -> Result<bool, JsValue> {
    use feishin_domain::*;
    use feishin_storage::BrowserPlaylistBackups;
    use std::{
        cell::Cell,
        task::{Context, Poll, Waker},
    };
    struct State {
        now: Cell<u64>,
        cancelled: bool,
    }
    impl Clock for State {
        fn now(&self) -> Milliseconds {
            Milliseconds(self.now.get())
        }
    }
    impl Cancellation for State {
        fn is_cancelled(&self) -> bool {
            self.cancelled
        }
    }
    let state = State {
        now: Cell::new(if action == "expired" { 11 } else { 0 }),
        cancelled: action == "cancelled",
    };
    let context = OperationContext {
        correlation: RequestContext {
            request: RequestId(41),
            generation: Generation(9),
            deadline: Milliseconds(10),
        },
        cancellation: &state,
        limits: OperationLimits {
            items: 100,
            request_bytes: if action == "small" { 1 } else { 256 * 1024 },
            response_bytes: 256 * 1024,
        },
    };
    let backup = PlaylistBackup {
        playlist: Scoped {
            server: ServerId::new("boundary"),
            id: PlaylistId::new("p"),
        },
        name: "Canonical".into(),
        songs: vec![SongId::new("same"), SongId::new("same")],
        captured_at: UnixMilliseconds(1234567890123),
    };
    let mut store =
        BrowserPlaylistBackups::open().map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
    let mut future = store.save(backup, &state, context);
    let result = match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => return Err(JsValue::from_str("unexpected pending browser save")),
    };
    match action {
        "cancelled" => Ok(result
            == Err(BackupFailure {
                cause: BackupError::Context(DomainError::Cancelled),
                may_have_written: false,
            })),
        "expired" => Ok(result
            == Err(BackupFailure {
                cause: BackupError::Context(DomainError::Deadline),
                may_have_written: false,
            })),
        "small" => Ok(result
            == Err(BackupFailure {
                cause: BackupError::Capacity,
                may_have_written: false,
            })),
        _ => result
            .map(|receipt| receipt.context == context.correlation)
            .map_err(|e| JsValue::from_str(&format!("{e:?}"))),
    }
}
