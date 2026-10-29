use super::*;
use std::time::Instant;
fn wait(c: &mut Controller, ctx: &Context, predicate: impl Fn(&Controller) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate(c) {
        assert!(Instant::now() < deadline, "worker did not finish");
        c.tick(ctx);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn load_save_close_and_reopen() {
    let root = tempfile::tempdir().unwrap();
    let ctx = Context::default();
    let mut c = Controller::new(root.path().to_path_buf()).unwrap();
    wait(&mut c, &ctx, |c| c.pending.is_none());
    assert_eq!(c.confirmed.as_ref().unwrap().revision, 0);
    c.draft.language = "de".into();
    c.draft.follow_system_theme = true;
    c.submit(
        Command::PreparePreferences {
            expected_revision: 0,
            preferences: c.draft.clone(),
        },
        Pending::Prepare,
    );
    c.close();
    wait(&mut c, &ctx, |c| c.stopped());
    let mut reopened = Controller::new(root.path().to_path_buf()).unwrap();
    wait(&mut reopened, &ctx, |c| c.pending.is_none());
    assert_eq!(reopened.draft.language, "de");
    assert!(reopened.draft.follow_system_theme);
    reopened.close();
    wait(&mut reopened, &ctx, |c| c.stopped());
}

#[test]
fn recovered_load_is_read_only_and_invalid_save_requires_reload() {
    use feishin_storage::{AtomicStore, NativeStore};
    let root = tempfile::tempdir().unwrap();
    let mut legacy = NativeStore::open(root.path()).unwrap();
    legacy
        .replace(br#"{"schema_version":1,"data":{"preferences":{"language":false}}}"#)
        .unwrap();
    let active = NativeStore::open_state(root.path()).unwrap();
    let active_path = active.path();
    let ctx = Context::default();
    let mut c = Controller::new(root.path().to_path_buf()).unwrap();
    wait(&mut c, &ctx, |c| c.pending.is_none());
    assert!(c.status.contains("recovered"));
    assert!(!active_path.exists());
    c.draft.language = "invalid value".into();
    c.submit(
        Command::PreparePreferences {
            expected_revision: 0,
            preferences: c.draft.clone(),
        },
        Pending::Prepare,
    );
    wait(&mut c, &ctx, |c| c.pending.is_none());
    assert!(c.confirmed.is_none());
    assert_eq!(c.draft.language, "invalid value");
    assert!(!active_path.exists());
    c.submit(Command::Load, Pending::Load);
    wait(&mut c, &ctx, |c| c.pending.is_none());
    assert!(c.confirmed.is_some());
    c.close();
    wait(&mut c, &ctx, |c| c.stopped());
    assert!(!active_path.exists());
}

#[test]
fn future_schema_is_preserved_and_save_disabled() {
    use feishin_storage::{AtomicStore, NativeStore};
    let root = tempfile::tempdir().unwrap();
    let mut active = NativeStore::open_state(root.path()).unwrap();
    let future = br#"{"schema_version":999}"#;
    active.replace(future).unwrap();
    let ctx = Context::default();
    let mut c = Controller::new(root.path().to_path_buf()).unwrap();
    wait(&mut c, &ctx, |c| c.pending.is_none());
    assert!(c.confirmed.is_none());
    c.close();
    wait(&mut c, &ctx, |c| c.stopped());
    assert_eq!(active.read().unwrap().unwrap(), future);
}

#[test]
fn failed_save_cancels_pending_close_until_error_can_be_seen() {
    let root = tempfile::tempdir().unwrap();
    let ctx = Context::default();
    let mut c = Controller::new(root.path().to_path_buf()).unwrap();
    wait(&mut c, &ctx, |c| c.pending.is_none());
    c.submit(
        Command::PreparePreferences {
            expected_revision: 0,
            preferences: Preferences {
                language: "invalid value".into(),
                follow_system_theme: false,
            },
        },
        Pending::Prepare,
    );
    c.close();
    wait(&mut c, &ctx, |c| c.pending.is_none());
    assert!(!c.closing && !c.stopped() && !c.shutdown);
    assert!(c.confirmed.is_none());
    assert!(c.status.contains("invalid"));
    // A second, explicit close acknowledges the now-visible error.
    c.close();
    wait(&mut c, &ctx, |c| c.stopped());
    assert!(
        !root
            .path()
            .join("feishin-rust-dev-p02/state-v1.json")
            .exists()
    );
}

#[test]
fn unrelated_and_wrong_kind_replies_do_not_publish_or_apply() {
    use feishin_storage::session::{Reply, RequestId, Response, SessionGeneration};
    let root = tempfile::tempdir().unwrap();
    let ctx = Context::default();
    let mut c = Controller::new(root.path().to_path_buf()).unwrap();
    let (id, _) = c.pending.unwrap();
    let wrong_id = RequestId(NonZeroU64::new(id.0.get() + 1).unwrap());
    c.handle_reply(
        &ctx,
        Reply {
            generation: c.generation,
            id: wrong_id,
            result: Ok(Response::Discarded),
        },
    );
    c.handle_reply(
        &ctx,
        Reply {
            generation: SessionGeneration(NonZeroU64::new(2).unwrap()),
            id,
            result: Ok(Response::Discarded),
        },
    );
    assert_eq!(c.pending.unwrap().0, id);
    assert!(c.confirmed.is_none());
    c.handle_reply(
        &ctx,
        Reply {
            generation: c.generation,
            id,
            result: Ok(Response::Discarded),
        },
    );
    assert!(c.pending.is_none());
    assert!(c.confirmed.is_none());
    assert!(c.status.contains("Unexpected"));
    c.close();
    wait(&mut c, &ctx, |c| c.stopped());
}
