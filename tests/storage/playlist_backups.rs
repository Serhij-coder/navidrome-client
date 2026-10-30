use feishin_domain::{PlaylistId, Revision, Scoped, ServerId};
use feishin_storage::{
    AtomicStore, MAX_BYTES,
    playlist_backups::{Backup, Error, MAX_BACKUPS, MAX_SONGS, Repository, Snapshot},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone, Default)]
struct Memory {
    bytes: Rc<RefCell<Option<Vec<u8>>>>,
    writes: Rc<Cell<usize>>,
    failure: Rc<Cell<u8>>,
}
impl AtomicStore for Memory {
    fn read(&mut self) -> Result<Option<Vec<u8>>, feishin_storage::Error> {
        Ok(self.bytes.borrow().clone())
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), feishin_storage::Error> {
        self.writes.set(self.writes.get() + 1);
        if self.failure.get() == 1 {
            return Err(feishin_storage::Error::Unavailable);
        }
        *self.bytes.borrow_mut() = Some(bytes.to_vec());
        if self.failure.get() == 2 {
            return Err(feishin_storage::Error::Io);
        }
        Ok(())
    }
}
fn backup(server: &str, playlist: &str) -> Backup {
    Backup {
        server_id: server.into(),
        playlist_id: playlist.into(),
        name: "Synthetic".into(),
        song_ids: vec!["same".into(), "other".into(), "same".into()],
        captured_at_unix_ms: 1234567890123,
    }
}
fn identity(server: &str, playlist: &str) -> Scoped<PlaylistId> {
    Scoped {
        server: ServerId::new(server),
        id: PlaylistId::new(playlist),
    }
}
#[test]
fn defaults_server_scopes_repeated_songs_revisions_and_explicit_retention() {
    let memory = Memory::default();
    let mut repo = Repository::new(memory.clone());
    assert_eq!(repo.load(), Ok(Snapshot::default()));
    assert_eq!(memory.writes.get(), 0);
    let a = backup("a:b", "c");
    let b = backup("a", "b:c");
    let same = backup("other", "c");
    assert_eq!(repo.save(&a, Revision(0)), Ok(Revision(1)));
    assert_eq!(repo.save(&b, Revision(1)), Ok(Revision(2)));
    assert_eq!(repo.save(&same, Revision(2)), Ok(Revision(3)));
    let snapshot = repo.load().unwrap();
    assert_eq!(snapshot.backups.len(), 3);
    assert_eq!(snapshot.get(&a.identity()), Some(&a));
    assert_eq!(snapshot.get(&b.identity()), Some(&b));
    assert_eq!(snapshot.get(&same.identity()), Some(&same));
    assert_eq!(snapshot.backups[0].song_ids, ["same", "other", "same"]);
    let before = memory.bytes.borrow().clone();
    assert_eq!(repo.save(&a, Revision(2)), Err(Error::Stale));
    assert_eq!(*memory.bytes.borrow(), before);
    let mut updated = a.clone();
    updated.song_ids = vec!["new".into(), "new".into()];
    assert_eq!(repo.save(&updated, Revision(3)), Ok(Revision(4)));
    assert_eq!(repo.load().unwrap().get(&a.identity()), Some(&updated));
    assert_eq!(repo.load().unwrap().get(&same.identity()), Some(&same));
    assert_eq!(
        repo.remove(&identity("missing", "c"), Revision(4)),
        Ok(Revision(4))
    );
    assert_eq!(repo.remove(&a.identity(), Revision(4)), Ok(Revision(5)));
    assert_eq!(repo.load().unwrap().get(&a.identity()), None);
    assert_eq!(repo.load().unwrap().get(&b.identity()), Some(&b));
}
#[test]
fn saved_fixture_and_corruption_future_source_and_duplicates_are_read_only() {
    let memory = Memory::default();
    *memory.bytes.borrow_mut() =
        Some(include_bytes!("../fixtures/p03/playlist-backups/v1.json").to_vec());
    let mut repo = Repository::new(memory.clone());
    let value = repo.load().unwrap();
    assert_eq!(value.revision, Revision(7));
    assert_eq!(value.backups[0].song_ids, ["same", "other", "same"]);
    assert_eq!(value.backups[0].captured_at_unix_ms, 1234567890123);
    assert_eq!(memory.writes.get(), 0);
    for (bytes, error) in [
        (
            include_bytes!("../fixtures/p03/playlist-backups/future.json").as_slice(),
            Error::UnsupportedVersion,
        ),
        (
            include_bytes!("../fixtures/p03/playlist-backups/partial.json").as_slice(),
            Error::InvalidBackup,
        ),
        (
            include_bytes!("../fixtures/p03/playlist-backups/duplicate.json").as_slice(),
            Error::InvalidDocument,
        ),
        (
            include_bytes!("../fixtures/p03/playlist-backups/source-unscoped.json").as_slice(),
            Error::InvalidDocument,
        ),
        (b"{malformed".as_slice(), Error::InvalidDocument),
    ] {
        *memory.bytes.borrow_mut() = Some(bytes.to_vec());
        let before = memory.bytes.borrow().clone();
        assert_eq!(repo.load(), Err(error));
        assert_eq!(repo.save(&backup("s", "p"), Revision(7)), Err(error));
        assert_eq!(repo.remove(&identity("s", "p"), Revision(7)), Err(error));
        assert_eq!(*memory.bytes.borrow(), before);
        assert_eq!(memory.writes.get(), 0);
    }
}
#[test]
fn bounds_encoding_failures_and_counter_exhaustion_preserve_prior_backups() {
    let memory = Memory::default();
    let mut repo = Repository::new(memory.clone());
    let mut revision = Revision(0);
    for i in 0..MAX_BACKUPS {
        revision = repo.save(&backup("s", &format!("p{i}")), revision).unwrap();
    }
    let before = memory.bytes.borrow().clone();
    assert_eq!(
        repo.save(&backup("s", "extra"), revision),
        Err(Error::Capacity)
    );
    assert_eq!(*memory.bytes.borrow(), before);
    let mut huge = backup("s", "p0");
    huge.song_ids = vec!["x".repeat(1024); 1000];
    assert_eq!(repo.save(&huge, revision), Err(Error::TooLarge));
    assert_eq!(*memory.bytes.borrow(), before);
    for which in 0..6 {
        let mut invalid = backup("s", "p0");
        match which {
            0 => invalid.server_id = " ".into(),
            1 => invalid.playlist_id = String::new(),
            2 => invalid.song_ids = vec!["".into()],
            3 => invalid.captured_at_unix_ms = 8_640_000_000_000_001,
            4 => invalid.song_ids = vec!["a".into(); MAX_SONGS + 1],
            _ => invalid.name = "x".repeat(8193),
        }
        assert_eq!(repo.save(&invalid, revision), Err(Error::InvalidBackup));
        assert_eq!(*memory.bytes.borrow(), before);
    }
    let mut value: serde_json::Value = serde_json::from_slice(before.as_ref().unwrap()).unwrap();
    value["revision"] = serde_json::json!(u64::MAX);
    *memory.bytes.borrow_mut() = Some(serde_json::to_vec(&value).unwrap());
    let before = memory.bytes.borrow().clone();
    assert_eq!(
        repo.save(&backup("s", "p0"), Revision(u64::MAX)),
        Err(Error::Exhausted)
    );
    assert_eq!(*memory.bytes.borrow(), before);
    *memory.bytes.borrow_mut() = Some(vec![b' '; MAX_BYTES + 1]);
    let before = memory.bytes.borrow().clone();
    assert_eq!(repo.load(), Err(Error::TooLarge));
    assert_eq!(
        repo.save(&backup("s", "p0"), Revision(0)),
        Err(Error::TooLarge)
    );
    assert_eq!(*memory.bytes.borrow(), before);
}
#[test]
fn failed_and_uncertain_atomic_writes_require_reconciliation() {
    let memory = Memory::default();
    let mut repo = Repository::new(memory.clone());
    assert_eq!(repo.save(&backup("s", "p"), Revision(0)), Ok(Revision(1)));
    let before = memory.bytes.borrow().clone();
    memory.failure.set(1);
    assert_eq!(
        repo.save(&backup("s", "q"), Revision(1)),
        Err(Error::Store(feishin_storage::Error::Unavailable))
    );
    assert_eq!(*memory.bytes.borrow(), before);
    memory.failure.set(2);
    assert_eq!(
        repo.save(&backup("s", "q"), Revision(1)),
        Err(Error::Store(feishin_storage::Error::Io))
    );
    let loaded = repo.load().unwrap();
    assert_eq!(loaded.revision, Revision(2));
    assert!(loaded.get(&identity("s", "q")).is_some());
    assert!(loaded.get(&identity("s", "p")).is_some());
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_reopen_isolated_value_interruption_and_path_errors() {
    use feishin_storage::NativeStore;
    let root = tempfile::tempdir().unwrap();
    let mut profile = NativeStore::open(root.path()).unwrap();
    profile.replace(b"original profile bytes").unwrap();
    let store = NativeStore::open_playlist_backups(root.path()).unwrap();
    let path = store.path();
    assert_eq!(path.file_name().unwrap(), "playlist-backups-v1.json");
    assert!(!path.exists());
    let mut repo = Repository::new(store);
    repo.save(&backup("s/../data", "p/../id"), Revision(0))
        .unwrap();
    drop(repo);
    std::fs::write(
        path.parent().unwrap().join("interrupted-backup.tmp"),
        b"partial not a repository value",
    )
    .unwrap();
    let mut repo = Repository::new(NativeStore::open_playlist_backups(root.path()).unwrap());
    assert_eq!(
        repo.load().unwrap().backups[0],
        backup("s/../data", "p/../id")
    );
    assert_eq!(profile.read().unwrap().unwrap(), b"original profile bytes");
    assert!(!root.path().join("data").exists());
    let raw = include_bytes!("../fixtures/p03/playlist-backups/future.json");
    std::fs::write(&path, raw).unwrap();
    assert_eq!(
        repo.save(&backup("s", "p"), Revision(1)),
        Err(Error::UnsupportedVersion)
    );
    assert_eq!(std::fs::read(&path).unwrap(), raw);
    assert!(matches!(
        NativeStore::open_playlist_backups("relative"),
        Err(feishin_storage::Error::UnsafePath)
    ));
    #[cfg(unix)]
    {
        let linked = root.path().join("linked");
        std::os::unix::fs::symlink(root.path(), &linked).unwrap();
        assert!(matches!(
            NativeStore::open_playlist_backups(&linked),
            Err(feishin_storage::Error::UnsafePath)
        ));
    }
}

#[test]
fn duplicate_document_members_never_discard_recovery_data() {
    let memory = Memory::default();
    let mut repo = Repository::new(memory.clone());
    let record = r#"{"serverId":"s","playlistId":"p","name":"Saved","songIds":["same","same"],"capturedAtUnixMs":1}"#;
    let documents = [
        format!(r#"{{"schema_version":1,"revision":7,"backups":[{record}],"backups":[]}}"#),
        format!(r#"{{"schema_version":2,"schema_version":1,"revision":7,"backups":[{record}]}}"#),
        format!(r#"{{"schema_version":1,"revision":6,"revision":7,"backups":[{record}]}}"#),
        format!(
            r#"{{"schema_version":1,"revision":7,"backups":[{}]}}"#,
            record.replace(
                r#""songIds":["same","same"]"#,
                r#""songIds":["same","same"],"songIds":[]"#
            )
        ),
        format!(
            r#"{{"schema_version":1,"revision":7,"backups":[{}]}}"#,
            record.replace(r#""serverId":"s""#, r#""serverId":"other","serverId":"s""#)
        ),
    ];
    for document in documents {
        *memory.bytes.borrow_mut() = Some(document.as_bytes().to_vec());
        assert_eq!(repo.load(), Err(Error::InvalidDocument));
        assert_eq!(
            repo.save(&backup("s", "p"), Revision(7)),
            Err(Error::InvalidDocument)
        );
        assert_eq!(
            repo.remove(&identity("s", "p"), Revision(7)),
            Err(Error::InvalidDocument)
        );
        assert_eq!(memory.bytes.borrow().as_deref(), Some(document.as_bytes()));
    }
    assert_eq!(memory.writes.get(), 0);
}
