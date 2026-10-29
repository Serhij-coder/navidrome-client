use feishin_storage::{
    AtomicStore, Error,
    session::{Command, Request, RequestId, Response, Session, SessionError, SessionGeneration},
    state::StateRepository,
};
use std::{cell::RefCell, num::NonZeroU64, rc::Rc, time::Duration};
#[derive(Clone, Default)]
struct Store(Rc<RefCell<Option<Vec<u8>>>>);
impl AtomicStore for Store {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.0.borrow().clone())
    }
    fn replace(&mut self, b: &[u8]) -> Result<(), Error> {
        *self.0.borrow_mut() = Some(b.to_vec());
        Ok(())
    }
}
fn generation(n: u64) -> SessionGeneration {
    SessionGeneration(NonZeroU64::new(n).unwrap())
}
fn call(s: &mut Session<Store, Store, Store>, c: Command) -> Result<Response, SessionError> {
    s.dispatch(
        Request {
            generation: generation(1),
            id: RequestId(NonZeroU64::new(1).unwrap()),
            deadline: Duration::from_secs(10),
            command: c,
        },
        Duration::ZERO,
        false,
    )
    .result
}
#[test]
fn reviewed_save_is_single_use_and_rollback_restores() {
    let store = Store::default();
    let mut s = Session::new(
        generation(1),
        StateRepository::new(store.clone(), Store::default(), Store::default()),
    );
    let bytes = serde_json::to_vec(&feishin_storage::imports::MigrationData::default()).unwrap();
    let Response::ActiveReview { token, .. } =
        call(&mut s, Command::PrepareSave(bytes)).ok().unwrap()
    else {
        panic!()
    };
    assert!(store.0.borrow().is_none());
    assert!(matches!(
        call(&mut s, Command::Apply(token)),
        Ok(Response::AppliedActive(_))
    ));
    assert!(matches!(
        call(&mut s, Command::Apply(token)),
        Err(SessionError::InvalidToken)
    ));
    let Response::ActiveReview { token, .. } = call(&mut s, Command::PrepareRollback).ok().unwrap()
    else {
        panic!()
    };
    assert!(matches!(
        call(&mut s, Command::Apply(token)),
        Ok(Response::AppliedActive(_))
    ));
}
#[test]
fn gates_and_superseded_tokens() {
    let mut s = Session::new(
        generation(1),
        StateRepository::new(Store::default(), Store::default(), Store::default()),
    );
    let request = Request {
        generation: generation(2),
        id: RequestId(NonZeroU64::new(1).unwrap()),
        deadline: Duration::from_secs(1),
        command: Command::Load,
    };
    assert!(matches!(
        s.dispatch(request, Duration::ZERO, false).result,
        Err(SessionError::WrongGeneration)
    ));
    let bytes = serde_json::to_vec(&feishin_storage::imports::MigrationData::default()).unwrap();
    let Response::ActiveReview { token, .. } = call(&mut s, Command::PrepareSave(bytes.clone()))
        .ok()
        .unwrap()
    else {
        panic!()
    };
    call(&mut s, Command::PrepareSave(bytes)).ok().unwrap();
    assert!(matches!(
        call(&mut s, Command::Apply(token)),
        Err(SessionError::InvalidToken)
    ));
    assert!(matches!(
        call(
            &mut s,
            Command::PrepareSave(vec![0; feishin_storage::MAX_BYTES + 1])
        ),
        Err(SessionError::TooLarge)
    ));
}

#[test]
fn cancellation_and_expiry_precede_store_access() {
    struct Fail;
    impl AtomicStore for Fail {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            panic!("unexpected read")
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            panic!("unexpected write")
        }
    }
    let mut s = Session::new(generation(1), StateRepository::new(Fail, Fail, Fail));
    let req = || Request {
        generation: generation(1),
        id: RequestId(NonZeroU64::new(1).unwrap()),
        deadline: Duration::from_secs(1),
        command: Command::Load,
    };
    assert!(matches!(
        s.dispatch(req(), Duration::ZERO, true).result,
        Err(SessionError::Cancelled)
    ));
    assert!(matches!(
        s.dispatch(req(), Duration::from_secs(1), false).result,
        Err(SessionError::Expired)
    ));
}
#[test]
fn uncertain_write_consumes_review_and_load_reconciles() {
    struct Uncertain(Rc<RefCell<Option<Vec<u8>>>>);
    impl AtomicStore for Uncertain {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            Ok(self.0.borrow().clone())
        }
        fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
            *self.0.borrow_mut() = Some(bytes.to_vec());
            Err(Error::Io)
        }
    }
    let mut s = Session::new(
        generation(1),
        StateRepository::new(Uncertain(Rc::default()), Store::default(), Store::default()),
    );
    let request = |command| Request {
        generation: generation(1),
        id: RequestId(NonZeroU64::new(1).unwrap()),
        deadline: Duration::from_secs(10),
        command,
    };
    let bytes = serde_json::to_vec(&feishin_storage::imports::MigrationData::default()).unwrap();
    let Response::ActiveReview { token, .. } = s
        .dispatch(request(Command::PrepareSave(bytes)), Duration::ZERO, false)
        .result
        .ok()
        .unwrap()
    else {
        panic!()
    };
    assert!(
        s.dispatch(request(Command::Apply(token)), Duration::ZERO, false)
            .result
            .is_err()
    );
    assert!(matches!(
        s.dispatch(request(Command::Apply(token)), Duration::ZERO, false)
            .result,
        Err(SessionError::InvalidToken)
    ));
    let Response::Load(loaded) = s
        .dispatch(request(Command::Load), Duration::ZERO, false)
        .result
        .ok()
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(loaded.snapshot.revision, 1);
}

#[test]
fn stage_adoption_and_cross_session_token() {
    use feishin_storage::session::OwnedInputs;
    let active = Store::default();
    let stage = Store::default();
    let mut s = Session::new(
        generation(1),
        StateRepository::new(active.clone(), stage.clone(), Store::default()),
    );
    let input = OwnedInputs {
        settings: Some(
            br#"{"version":34,"general":{"language":"uk","followSystemTheme":true}}"#.to_vec(),
        ),
        ..Default::default()
    };
    let Response::StageReview { token, data, .. } =
        call(&mut s, Command::PrepareStage(input)).ok().unwrap()
    else {
        panic!()
    };
    assert_eq!(data.profile.preferences.language, "uk");
    assert!(stage.0.borrow().is_none());
    let mut other = Session::new(
        generation(2),
        StateRepository::new(Store::default(), Store::default(), Store::default()),
    );
    let r = Request {
        generation: generation(2),
        id: RequestId(NonZeroU64::new(1).unwrap()),
        deadline: Duration::from_secs(10),
        command: Command::Apply(token),
    };
    assert!(matches!(
        other.dispatch(r, Duration::ZERO, false).result,
        Err(SessionError::InvalidToken)
    ));
    assert!(matches!(
        call(&mut s, Command::Apply(token)),
        Ok(Response::AppliedStage(_))
    ));
    let Response::ActiveReview { token, after, .. } =
        call(&mut s, Command::PrepareAdoption).ok().unwrap()
    else {
        panic!()
    };
    assert_eq!(after.data.profile.preferences.language, "uk");
    assert!(active.0.borrow().is_none());
    assert!(matches!(
        call(&mut s, Command::Apply(token)),
        Ok(Response::AppliedActive(_))
    ));
}

#[test]
fn portable_bounds_malformed_save_and_recovery() {
    use feishin_storage::{
        imports::{MAX_INPUT_BYTES, SettingsCategory},
        session::OwnedInputs,
    };
    let legacy = Store::default();
    *legacy.0.borrow_mut() =
        Some(br#"{"schema_version":1,"data":{"preferences":{"language":"!"}}}"#.to_vec());
    let mut s = Session::new(
        generation(1),
        StateRepository::new(Store::default(), Store::default(), legacy),
    );
    let Response::Load(loaded) = call(&mut s, Command::Load).ok().unwrap() else {
        panic!()
    };
    assert!(!loaded.recovered.is_empty());
    assert!(matches!(
        call(&mut s, Command::PrepareSave(b"{".to_vec())),
        Err(SessionError::InvalidSave)
    ));
    let mut input = OwnedInputs {
        auth: Some(vec![0; MAX_INPUT_BYTES]),
        ..Default::default()
    };
    input
        .split_categories
        .insert(SettingsCategory::General, vec![0]);
    assert!(matches!(
        call(&mut s, Command::PrepareStage(input)),
        Err(SessionError::TooLarge)
    ));
}
