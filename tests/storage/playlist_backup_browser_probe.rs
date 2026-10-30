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
