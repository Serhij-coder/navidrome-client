#[cfg(not(target_arch = "wasm32"))]
use feishin_storage::NativeStore;
use feishin_storage::{
    AtomicStore, Error, MAX_BYTES,
    imports::{CopiedInputs, MigrationData},
    state::{StateError, StateRepository},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<Inner>>);
#[derive(Default)]
struct Inner {
    bytes: Option<Vec<u8>>,
    writes: usize,
    fail: bool,
    visible: bool,
}
impl AtomicStore for Memory {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.0.borrow().bytes.clone())
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let mut inner = self.0.borrow_mut();
        inner.writes += 1;
        if !inner.fail || inner.visible {
            inner.bytes = Some(bytes.to_vec());
        }
        if inner.fail { Err(Error::Io) } else { Ok(()) }
    }
}
fn repo() -> (
    StateRepository<Memory, Memory, Memory>,
    Memory,
    Memory,
    Memory,
) {
    let a = Memory::default();
    let j = Memory::default();
    let p = Memory::default();
    (
        StateRepository::new(a.clone(), j.clone(), p.clone()),
        a,
        j,
        p,
    )
}
#[test]
fn missing_legacy_recovery_and_save_are_read_only_until_apply() {
    let (mut r, a, _, p) = repo();
    assert_eq!(r.load().unwrap().snapshot.revision, 0);
    p.0.borrow_mut().bytes =
        Some(br#"{"schema_version":1,"data":{"preferences":{"language":false}}}"#.to_vec());
    let loaded = r.load().unwrap();
    assert_eq!(loaded.recovered.len(), 1);
    let save = r.prepare_save(&loaded.snapshot.data).unwrap();
    assert_eq!(a.0.borrow().writes, 0);
    assert_eq!(r.apply(&save).unwrap().revision, 1);
    assert_eq!(r.apply(&save), Err(StateError::Conflict));
    let again = r.prepare_save(&loaded.snapshot.data).unwrap();
    r.apply(&again).unwrap();
    assert_eq!(a.0.borrow().writes, 1);
}
#[test]
fn stage_adoption_undo_redo_and_stale_guards() {
    let (mut r, a, j, p) = repo();
    p.0.borrow_mut().bytes =
        Some(br#"{"schema_version":1,"data":{"preferences":{"language":"de"}}}"#.to_vec());
    assert_eq!(r.prepare_adoption().err(), Some(StateError::EmptyStage));
    let input = CopiedInputs::default();
    let staged = r.journal_mut().prepare(&input).unwrap();
    r.journal_mut().apply(&staged).unwrap();
    let adopt = r.prepare_adoption().unwrap();
    assert_eq!(adopt.before().data.profile.preferences.language, "de");
    p.0.borrow_mut().bytes = None;
    assert_eq!(r.apply(&adopt), Err(StateError::Conflict));
    p.0.borrow_mut().bytes =
        Some(br#"{"schema_version":1,"data":{"preferences":{"language":"de"}}}"#.to_vec());
    let adopt = r.prepare_adoption().unwrap();
    r.apply(&adopt).unwrap();
    let undo = r.prepare_rollback().unwrap();
    r.apply(&undo).unwrap();
    assert_eq!(
        r.load().unwrap().snapshot.data.profile.preferences.language,
        "de"
    );
    let redo = r.prepare_rollback().unwrap();
    r.apply(&redo).unwrap();
    p.0.borrow_mut().bytes = Some(b"broken".to_vec());
    assert!(r.load().is_ok());
    let stale = r.prepare_adoption().unwrap();
    j.0.borrow_mut().bytes = None;
    assert_eq!(r.apply(&stale), Err(StateError::Conflict));
    let stale = r.prepare_rollback().unwrap();
    let mut edit = r.load().unwrap().snapshot.data;
    edit.profile.preferences.language = "fr".into();
    let save = r.prepare_save(&edit).unwrap();
    r.apply(&save).unwrap();
    assert_eq!(r.apply(&stale), Err(StateError::Conflict));
    assert!(a.0.borrow().writes >= 4);
}
#[test]
fn corrupt_future_capacity_and_uncertain_replace_fail_closed() {
    let (mut r, a, _, _) = repo();
    for raw in [
        br#"{}"#.as_slice(),
        br#"{"schema_version":2}"#,
        br#"{"schema_version":1,"current":{},"backup":{}}"#,
    ] {
        a.0.borrow_mut().bytes = Some(raw.to_vec());
        assert!(r.load().is_err());
        assert!(r.prepare_save(&MigrationData::default()).is_err());
    }
    a.0.borrow_mut().bytes = Some(vec![b' '; MAX_BYTES + 1]);
    assert_eq!(r.load(), Err(StateError::Storage(Error::TooLarge)));
    a.0.borrow_mut().bytes = None;
    let save = r.prepare_save(&MigrationData::default()).unwrap();
    a.0.borrow_mut().fail = true;
    assert_eq!(r.apply(&save), Err(StateError::Storage(Error::Io)));
    assert_eq!(r.load().unwrap().snapshot.revision, 0);
    a.0.borrow_mut().visible = true;
    assert_eq!(r.apply(&save), Err(StateError::Storage(Error::Io)));
    assert_eq!(r.load().unwrap().snapshot.revision, 1);
    assert_eq!(r.apply(&save), Err(StateError::Conflict));
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_namespaces_reopen_and_interrupted_temp() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let active = NativeStore::open_state(&root).unwrap();
    let path = active.path();
    let mut profile_store = NativeStore::open(&root).unwrap();
    let profile_path = profile_store.path();
    profile_store
        .replace(br#"{"schema_version":1,"data":{"preferences":{"language":"de"}}}"#)
        .unwrap();
    let original_profile = std::fs::read(&profile_path).unwrap();
    let stage_path = NativeStore::open_imports(&root).unwrap().path();
    let mut r = StateRepository::new(
        active,
        NativeStore::open_imports(&root).unwrap(),
        NativeStore::open(&root).unwrap(),
    );
    let staged = r
        .journal_mut()
        .prepare(&CopiedInputs {
            auth: Some(include_bytes!("../fixtures/p02/imports/auth-v2.json")),
            player: Some(include_bytes!("../fixtures/p02/imports/player-v3.json")),
            timestamp: Some(b"12.5"),
            ..Default::default()
        })
        .unwrap();
    r.journal_mut().apply(&staged).unwrap();
    let original_stage = std::fs::read(&stage_path).unwrap();
    let adoption = r.prepare_adoption().unwrap();
    let expected = r.apply(&adoption).unwrap();
    std::fs::write(path.parent().unwrap().join("interrupted.tmp"), b"broken").unwrap();
    let mut reopened = StateRepository::new(
        NativeStore::open_state(&root).unwrap(),
        NativeStore::open_imports(&root).unwrap(),
        NativeStore::open(&root).unwrap(),
    );
    assert_eq!(reopened.load().unwrap().snapshot, expected);
    let undo = reopened.prepare_rollback().unwrap();
    assert_eq!(
        reopened
            .apply(&undo)
            .unwrap()
            .data
            .profile
            .preferences
            .language,
        "de"
    );
    let redo = reopened.prepare_rollback().unwrap();
    assert_eq!(reopened.apply(&redo).unwrap().data, expected.data);
    assert_eq!(std::fs::read(profile_path).unwrap(), original_profile);
    assert_eq!(std::fs::read(stage_path).unwrap(), original_stage);
}

#[test]
fn failed_rollback_preserves_snapshot_and_can_retry_only_before_visible_replacement() {
    let (mut r, a, _, _) = repo();
    let mut data = MigrationData::default();
    data.profile.preferences.language = "de".into();
    let save = r.prepare_save(&data).unwrap();
    let before = r.apply(&save).unwrap();
    let rollback = r.prepare_rollback().unwrap();
    a.0.borrow_mut().fail = true;
    assert_eq!(r.apply(&rollback), Err(StateError::Storage(Error::Io)));
    assert_eq!(r.load().unwrap().snapshot, before);
    a.0.borrow_mut().visible = true;
    assert_eq!(r.apply(&rollback), Err(StateError::Storage(Error::Io)));
    assert_eq!(r.load().unwrap().snapshot.data, MigrationData::default());
    assert_eq!(r.apply(&rollback), Err(StateError::Conflict));
}

#[test]
fn tracked_queue_occurrences_survive_adoption_and_reopen() {
    let (mut r, a, j, p) = repo();
    let input = CopiedInputs {
        auth: Some(include_bytes!("../fixtures/p02/imports/auth-v2.json")),
        player: Some(include_bytes!("../fixtures/p02/imports/player-v3.json")),
        timestamp: Some(b"12.5"),
        ..Default::default()
    };
    let stage = r.journal_mut().prepare(&input).unwrap();
    r.journal_mut().apply(&stage).unwrap();
    let prepared = r.prepare_adoption().unwrap();
    r.apply(&prepared).unwrap();
    let mut reopened = StateRepository::new(a, j, p);
    let data = reopened.load().unwrap().snapshot.data;
    let queue = data.queue.unwrap();
    assert_eq!(queue.occurrences.len(), 2);
    assert_eq!(queue.occurrences[0].song_id, queue.occurrences[1].song_id);
    assert_ne!(
        queue.occurrences[0].occurrence_id,
        queue.occurrences[1].occurrence_id
    );
}

#[test]
fn invalid_backup_and_revision_exhaustion_are_protected() {
    let (mut r, a, _, _) = repo();
    let save = r.prepare_save(&MigrationData::default()).unwrap();
    r.apply(&save).unwrap();
    let mut raw: serde_json::Value =
        serde_json::from_slice(a.0.borrow().bytes.as_ref().unwrap()).unwrap();
    raw["backup"]["data"]["profile"]["preferences"]["language"] = "".into();
    a.0.borrow_mut().bytes = Some(serde_json::to_vec(&raw).unwrap());
    assert_eq!(r.load(), Err(StateError::InvalidState));
    raw["backup"]["data"]["profile"]["preferences"]["language"] = "en".into();
    raw["current"]["revision"] = u64::MAX.into();
    raw["backup"]["revision"] = (u64::MAX - 1).into();
    a.0.borrow_mut().bytes = Some(serde_json::to_vec(&raw).unwrap());
    let mut data = r.load().unwrap().snapshot.data;
    data.profile.preferences.language = "de".into();
    assert_eq!(r.prepare_save(&data).err(), Some(StateError::Exhausted));
}

#[test]
fn preparation_bounds_current_and_backup_together() {
    use feishin_storage::imports::{QueueOccurrence, QueueSnapshot};
    let (mut r, a, _, _) = repo();
    let mut data = MigrationData {
        queue: Some(QueueSnapshot {
            occurrences: (0..900)
                .map(|i| QueueOccurrence {
                    occurrence_id: i.to_string(),
                    song_id: "song".into(),
                    server_id: "server".into(),
                    title: "x".repeat(100),
                    duration_ms: 1,
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let save = r.prepare_save(&data).unwrap();
    r.apply(&save).unwrap();
    data.profile.preferences.language = "de".into();
    assert_eq!(
        r.prepare_save(&data).err(),
        Some(StateError::Storage(Error::TooLarge))
    );
    assert_eq!(a.0.borrow().writes, 1);
}

#[test]
fn oversized_local_input_is_rejected_without_changing_the_active_snapshot() {
    use feishin_storage::imports::{QueueOccurrence, QueueSnapshot};
    let (mut r, a, _, _) = repo();
    let saved = r.prepare_save(&MigrationData::default()).unwrap();
    let before = r.apply(&saved).unwrap();
    let mut data = before.data.clone();
    data.queue = Some(QueueSnapshot {
        occurrences: vec![QueueOccurrence {
            occurrence_id: "first".into(),
            song_id: "song".into(),
            server_id: "server".into(),
            title: "x".repeat(1025),
            duration_ms: 1,
        }],
        ..Default::default()
    });
    assert_eq!(r.prepare_save(&data).err(), Some(StateError::InvalidState));
    assert_eq!(r.load().unwrap().snapshot, before);
    assert_eq!(a.0.borrow().writes, 1);
}
