use feishin_storage::{AtomicStore, Error, MAX_BYTES, imports::CopiedInputs, journal::*};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Memory {
    bytes: Rc<RefCell<Option<Vec<u8>>>>,
    fail: Rc<RefCell<bool>>,
    writes: Rc<RefCell<usize>>,
    uncertain: Rc<RefCell<bool>>,
}
impl AtomicStore for Memory {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.bytes.borrow().clone())
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if *self.fail.borrow() {
            return Err(Error::Unavailable);
        }
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        *self.bytes.borrow_mut() = Some(bytes.to_vec());
        *self.writes.borrow_mut() += 1;
        if *self.uncertain.borrow() {
            return Err(Error::Io);
        }
        Ok(())
    }
}
fn settings(language: &'static str) -> CopiedInputs<'static> {
    let bytes: &'static [u8] = match language {
        "uk" => br#"{"version":34,"general":{"language":"uk","followSystemTheme":true}}"#,
        _ => br#"{"version":34,"general":{"language":"en","followSystemTheme":false}}"#,
    };
    CopiedInputs {
        settings: Some(bytes),
        ..CopiedInputs::default()
    }
}
#[test]
fn reviewed_candidate_apply_idempotence_and_rollback_are_atomic() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    assert_eq!(
        repo.load().unwrap(),
        Snapshot {
            revision: 0,
            data: None
        }
    );
    assert!(matches!(
        repo.prepare_rollback(),
        Err(JournalError::NoBackup)
    ));
    let prepared = repo.prepare(&settings("uk")).unwrap();
    assert_eq!(prepared.preview().data().profile.preferences.language, "uk");
    assert!(store.bytes.borrow().is_none());
    assert_eq!(*store.writes.borrow(), 0);
    let first = repo.apply(&prepared).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(
        first.data.as_ref().unwrap().profile.preferences.language,
        "uk"
    );
    assert_eq!(repo.apply(&prepared), Err(JournalError::Conflict));
    let again = repo.prepare(&settings("uk")).unwrap();
    assert_eq!(repo.apply(&again).unwrap(), first);
    assert_eq!(*store.writes.borrow(), 1);
    let rollback = repo.prepare_rollback().unwrap();
    assert!(rollback.restored().is_none());
    let empty = repo.rollback(&rollback).unwrap();
    assert_eq!(
        empty,
        Snapshot {
            revision: 2,
            data: None
        }
    );
    assert_eq!(repo.rollback(&rollback), Err(JournalError::Conflict));
    let redo = repo.prepare_rollback().unwrap();
    assert_eq!(redo.restored(), first.data.as_ref());
    let restored = repo.rollback(&redo).unwrap();
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.data, first.data);
}
#[test]
fn failed_import_and_rollback_preserve_target_and_are_retryable() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let first = repo.prepare(&settings("uk")).unwrap();
    repo.apply(&first).unwrap();
    let before = store.bytes.borrow().clone();
    let second = repo.prepare(&settings("en")).unwrap();
    *store.fail.borrow_mut() = true;
    assert_eq!(
        repo.apply(&second),
        Err(JournalError::Storage(Error::Unavailable))
    );
    assert_eq!(*store.bytes.borrow(), before);
    assert_eq!(
        repo.load()
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .profile
            .preferences
            .language,
        "uk"
    );
    *store.fail.borrow_mut() = false;
    repo.apply(&second).unwrap();
    let rollback = repo.prepare_rollback().unwrap();
    let before = store.bytes.borrow().clone();
    *store.fail.borrow_mut() = true;
    assert_eq!(
        repo.rollback(&rollback),
        Err(JournalError::Storage(Error::Unavailable))
    );
    assert_eq!(*store.bytes.borrow(), before);
    *store.fail.borrow_mut() = false;
    assert_eq!(
        repo.rollback(&rollback)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .profile
            .preferences
            .language,
        "uk"
    );
}
#[test]
fn stale_plans_reject_intervening_changes_before_writing() {
    let store = Memory::default();
    let mut first = JournalRepository::new(store.clone());
    let mut second = JournalRepository::new(store.clone());
    let a = first.prepare(&settings("uk")).unwrap();
    let stale = second.prepare(&settings("en")).unwrap();
    first.apply(&a).unwrap();
    let rollback = first.prepare_rollback().unwrap();
    let b = second.prepare(&settings("en")).unwrap();
    second.apply(&b).unwrap();
    let before = store.bytes.borrow().clone();
    assert_eq!(second.apply(&stale), Err(JournalError::Conflict));
    assert_eq!(first.rollback(&rollback), Err(JournalError::Conflict));
    assert_eq!(*store.bytes.borrow(), before);
}
#[test]
fn corrupt_future_invalid_backup_and_revision_exhaustion_fail_closed() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    for (raw, error) in [
        (b"{malformed".as_slice(), JournalError::InvalidJournal),
        (br#"{"schema_version":999,"current":null}"#.as_slice(), JournalError::UnsupportedSchema),
        (br#"{"schema_version":1,"current":{"revision":2,"data":null},"backup":{"revision":2,"data":null}}"#.as_slice(), JournalError::InvalidJournal),
        (br#"{"schema_version":1,"current":{"revision":1,"data":null},"backup":null,"credential":"synthetic"}"#.as_slice(), JournalError::InvalidJournal),
    ] {
        *store.bytes.borrow_mut() = Some(raw.to_vec());
        assert_eq!(repo.load(), Err(error));
        assert!(matches!(repo.prepare(&settings("uk")), Err(e) if e == error));
        assert!(matches!(repo.prepare_rollback(), Err(e) if e == error));
        assert_eq!(*store.bytes.borrow(), Some(raw.to_vec()));
    }
    *store.bytes.borrow_mut() = Some(br#"{"schema_version":1,"current":{"revision":18446744073709551615,"data":null},"backup":{"revision":18446744073709551614,"data":null}}"#.to_vec());
    assert!(matches!(
        repo.prepare(&settings("uk")),
        Err(JournalError::Exhausted)
    ));
    let rollback = repo.prepare_rollback().unwrap();
    assert_eq!(repo.rollback(&rollback), Err(JournalError::Exhausted));
    assert_eq!(*store.writes.borrow(), 0);
    *store.bytes.borrow_mut() = Some(vec![0; MAX_BYTES + 1]);
    assert_eq!(repo.load(), Err(JournalError::Storage(Error::TooLarge)));
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_imports_survive_reopen_rollback_and_interrupted_temporary_files() {
    use feishin_storage::{NativeStore, ProfileRepository};
    let root = tempfile::tempdir().unwrap();
    let canonical = root.path().canonicalize().unwrap();
    let profile_store = NativeStore::open(&canonical).unwrap();
    let profile_path = profile_store.path();
    let mut profiles = ProfileRepository::new(profile_store);
    profiles.save(&Default::default()).unwrap();
    let original = std::fs::read(&profile_path).unwrap();
    let import_store = NativeStore::open_imports(&canonical).unwrap();
    let path = import_store.path();
    assert_ne!(path, profile_path);
    let mut repo = JournalRepository::new(import_store);
    let prepared = repo.prepare(&settings("uk")).unwrap();
    let expected = repo.apply(&prepared).unwrap();
    std::fs::write(
        path.parent().unwrap().join(".interrupted-import"),
        b"partial",
    )
    .unwrap();
    let mut reopened = JournalRepository::new(NativeStore::open_imports(&canonical).unwrap());
    assert_eq!(reopened.load().unwrap(), expected);
    let rollback = reopened.prepare_rollback().unwrap();
    assert!(reopened.rollback(&rollback).unwrap().data.is_none());
    let redo = reopened.prepare_rollback().unwrap();
    assert_eq!(reopened.rollback(&redo).unwrap().data, expected.data);
    assert_eq!(std::fs::read(profile_path).unwrap(), original);
}

#[test]
fn durability_error_after_visible_replace_requires_reconciliation() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let prepared = repo.prepare(&settings("uk")).unwrap();
    *store.uncertain.borrow_mut() = true;
    assert_eq!(repo.apply(&prepared), Err(JournalError::Storage(Error::Io)));
    assert_eq!(repo.load().unwrap().revision, 1);
    assert_eq!(repo.apply(&prepared), Err(JournalError::Conflict));
    *store.uncertain.borrow_mut() = false;
    let rollback = repo.prepare_rollback().unwrap();
    assert!(repo.rollback(&rollback).unwrap().data.is_none());
}
#[test]
fn backup_capacity_is_checked_before_presenting_an_import() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let mut profiles = serde_json::Map::new();
    for i in 0..64 {
        profiles.insert(
            i.to_string(),
            serde_json::json!({
                "id":i.to_string(), "name":"Fixture", "type":"navidrome",
                "url":format!("https://example.test/{}", "x".repeat(3300))
            }),
        );
    }
    let auth = serde_json::to_vec(&serde_json::json!({
        "version":2,"state":{"serverList":profiles,"currentServer":null}
    }))
    .unwrap();
    let plan = repo
        .prepare(&CopiedInputs {
            auth: Some(&auth),
            ..CopiedInputs::default()
        })
        .unwrap();
    repo.apply(&plan).unwrap();
    let before = store.bytes.borrow().clone();
    assert!(before.as_ref().unwrap().len() > MAX_BYTES / 2);
    assert!(matches!(
        repo.prepare(&settings("uk")),
        Err(JournalError::Storage(Error::TooLarge))
    ));
    assert_eq!(*store.bytes.borrow(), before);
}
#[test]
fn corrupted_backup_data_is_never_used_as_a_rollback_source() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let first = repo.prepare(&settings("uk")).unwrap();
    repo.apply(&first).unwrap();
    let second = repo.prepare(&settings("en")).unwrap();
    repo.apply(&second).unwrap();
    let mut envelope: serde_json::Value =
        serde_json::from_slice(store.bytes.borrow().as_ref().unwrap()).unwrap();
    envelope["backup"]["data"]["profile"]["current_server"] = serde_json::json!("orphan");
    let bytes = serde_json::to_vec(&envelope).unwrap();
    *store.bytes.borrow_mut() = Some(bytes.clone());
    assert_eq!(repo.load(), Err(JournalError::InvalidJournal));
    assert!(matches!(
        repo.prepare_rollback(),
        Err(JournalError::InvalidJournal)
    ));
    assert_eq!(*store.bytes.borrow(), Some(bytes));
}

#[test]
fn historical_transforms_are_the_exact_reviewed_applied_and_rollback_data() {
    use feishin_storage::imports::{AutoDjStrategy, ColumnId};
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let initial = repo.prepare(&settings("uk")).unwrap();
    let previous = repo.apply(&initial).unwrap();
    let raw = br#"{"version":20,"state":{"lists":{"song":{"table":{"columns":[{"id":"titleCombined","align":"end","autoSize":false,"isEnabled":true,"pinned":null,"width":73}]}}},"autoDJ":{"mode":"old","songStrategy":"similar_forward"}}}"#;
    let source_before = raw.to_vec();
    let input = CopiedInputs {
        settings: Some(raw),
        ..CopiedInputs::default()
    };
    let prepared = repo.prepare(&input).unwrap();
    let reviewed = prepared.preview().data().clone();
    assert!(
        reviewed.columns[0]
            .columns
            .iter()
            .any(|c| c.id == ColumnId::TitleArtist)
    );
    assert_eq!(reviewed.columns[0].columns[0].width.as_u64(), Some(73));
    assert_eq!(
        reviewed.auto_dj.as_ref().unwrap().song_strategy,
        AutoDjStrategy::Similar
    );
    assert_eq!(repo.load().unwrap(), previous);
    let applied = repo.apply(&prepared).unwrap();
    assert_eq!(applied.data, Some(reviewed));
    let before = store.bytes.borrow().clone();
    let again = repo.prepare(&input).unwrap();
    assert_eq!(repo.apply(&again).unwrap(), applied);
    assert_eq!(*store.bytes.borrow(), before);
    let rollback = repo.prepare_rollback().unwrap();
    assert_eq!(repo.rollback(&rollback).unwrap().data, previous.data);
    assert_eq!(raw.as_slice(), source_before);
}

#[test]
fn reset_preview_apply_and_rollback_preserve_profile_and_queue_state() {
    use feishin_storage::imports::Code;
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let initial=br#"{"version":34,"general":{"language":"uk","followSystemTheme":true},"autoDJ":{"albumStrategy":"library_random","allowDuplicates":true,"enabled":true,"itemCount":9,"mode":"albums","onlySimilar":false,"songStrategy":"library_random","timing":2},"lists":{"song":{"table":{"columns":[{"id":"duration","align":"end","autoSize":false,"isEnabled":true,"pinned":null,"width":73}]}}}}"#;
    let input = CopiedInputs {
        settings: Some(initial),
        auth: Some(include_bytes!("../fixtures/p02/imports/auth-v2.json")),
        player: Some(include_bytes!("../fixtures/p02/imports/player-v3.json")),
        timestamp: Some(b"12.5"),
        ..Default::default()
    };
    let prepared = repo.prepare(&input).unwrap();
    let previous = repo.apply(&prepared).unwrap();
    let raw=br#"{"version":10,"state":{"general":{"sidebarItems":[]},"autoDJ":{"credential":"SYNTHETIC-RESET-CANARY"},"lists":null}}"#;
    let before = raw.to_vec();
    let input = CopiedInputs {
        settings: Some(raw),
        ..Default::default()
    };
    let prepared = repo.prepare(&input).unwrap();
    let expected = prepared.preview().data().clone();
    assert!(
        prepared
            .preview()
            .report()
            .diagnostics
            .iter()
            .any(|d| d.code == Code::SettingsReset)
    );
    assert_eq!(repo.load().unwrap(), previous);
    let old = previous.data.as_ref().unwrap();
    assert_eq!(expected.profile.profiles, old.profile.profiles);
    assert_eq!(expected.profile.current_server, old.profile.current_server);
    assert_eq!(expected.queue, old.queue);
    assert_eq!(expected.profile.preferences, Default::default());
    assert!(expected.columns.is_empty());
    assert!(!expected.auto_dj.as_ref().unwrap().enabled);
    *store.fail.borrow_mut() = true;
    assert_eq!(
        repo.apply(&prepared),
        Err(JournalError::Storage(Error::Unavailable))
    );
    assert_eq!(repo.load().unwrap(), previous);
    *store.fail.borrow_mut() = false;
    let applied = repo.apply(&prepared).unwrap();
    assert_eq!(applied.data, Some(expected));
    let writes = *store.writes.borrow();
    let repeated = repo.prepare(&input).unwrap();
    assert_eq!(repo.apply(&repeated).unwrap(), applied);
    assert_eq!(*store.writes.borrow(), writes);
    assert!(
        !String::from_utf8(store.bytes.borrow().as_ref().unwrap().clone())
            .unwrap()
            .contains("SYNTHETIC-RESET-CANARY")
    );
    let rollback = repo.prepare_rollback().unwrap();
    assert_eq!(rollback.restored(), previous.data.as_ref());
    assert_eq!(repo.rollback(&rollback).unwrap().data, previous.data);
    assert_eq!(raw.as_slice(), before);
}
#[test]
fn malformed_reset_never_creates_a_prepared_mutation() {
    let store = Memory::default();
    let mut repo = JournalRepository::new(store.clone());
    let valid = repo.prepare(&settings("uk")).unwrap();
    let before = repo.apply(&valid).unwrap();
    let writes = *store.writes.borrow();
    for raw in [
        br#"{"version":10,"state":{}}"#.as_slice(),
        br#"{"version":11,"state":null}"#.as_slice(),
    ] {
        assert!(matches!(
            repo.prepare(&CopiedInputs {
                settings: Some(raw),
                ..Default::default()
            }),
            Err(JournalError::Import(
                feishin_storage::imports::ImportError::InvalidDocument
            ))
        ));
        assert_eq!(repo.load().unwrap(), before);
        assert_eq!(*store.writes.borrow(), writes);
    }
}
