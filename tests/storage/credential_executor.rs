use feishin_domain::Secret;
use feishin_storage::{
    BackendKind, Profile,
    credential_executor::{
        Command, DispatchError, Dispatcher, Generation, Request, RequestId, Response,
    },
    credentials::{
        CredentialError, CredentialKey, CredentialSlot, MAX_SECRET_BYTES, PersistentCredentials,
    },
};
use std::{cell::RefCell, num::NonZeroU64, rc::Rc, time::Duration};
fn generation() -> Generation {
    Generation(NonZeroU64::new(1).unwrap())
}
fn key() -> CredentialKey {
    CredentialKey::new(
        "test",
        &Profile {
            id: "id".into(),
            name: "name".into(),
            endpoint: "https://example.test".into(),
            backend: BackendKind::Subsonic,
        },
        "account",
        CredentialSlot::LoginPassword,
    )
    .unwrap()
}
fn request(command: Command, id: u64) -> Request {
    Request {
        generation: generation(),
        id: RequestId(NonZeroU64::new(id).unwrap()),
        latest_start: Duration::from_secs(2),
        command,
    }
}
#[derive(Default)]
struct Provider {
    calls: Rc<RefCell<usize>>,
    value: Option<Vec<u8>>,
    fail: bool,
}
impl PersistentCredentials for Provider {
    fn persist(&mut self, _: &CredentialKey, s: &Secret) -> Result<(), CredentialError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CredentialError::Backend);
        }
        self.value = Some(s.expose().to_vec());
        Ok(())
    }
    fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
        *self.calls.borrow_mut() += 1;
        Ok(self.value.clone().map(Secret::new))
    }
    fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CredentialError::Backend);
        }
        self.value = None;
        Ok(())
    }
}
#[test]
fn local_non_send_explicit_flow_and_gates() {
    let calls = Rc::new(RefCell::new(0));
    let mut d = Dispatcher::with_backend(
        generation(),
        Provider {
            calls: calls.clone(),
            ..Provider::default()
        },
    );
    let reply = d.dispatch(
        request(Command::Put(key(), Secret::new(vec![1])), 1),
        Duration::ZERO,
        false,
    );
    assert_eq!(reply.id.0.get(), 1);
    assert!(matches!(reply.result, Ok(Response::Done)));
    assert_eq!(*calls.borrow(), 0);
    assert!(matches!(
        d.dispatch(request(Command::Remember(key()), 2), Duration::ZERO, false)
            .result,
        Ok(Response::Done)
    ));
    d.dispatch(request(Command::Clear, 3), Duration::ZERO, false);
    assert!(matches!(
        d.dispatch(request(Command::Restore(key()), 4), Duration::ZERO, false)
            .result,
        Ok(Response::Restored(true))
    ));
    assert!(
        matches!(d.dispatch(request(Command::Get(key()), 5), Duration::ZERO, false).result, Ok(Response::Secret(Some(s))) if s.expose() == [1])
    );
    assert!(matches!(
        d.dispatch(request(Command::Forget(key()), 6), Duration::ZERO, false)
            .result,
        Ok(Response::Done)
    ));
    assert!(matches!(
        d.dispatch(request(Command::Remember(key()), 7), Duration::ZERO, true)
            .result,
        Err(DispatchError::Cancelled)
    ));
    assert!(matches!(
        d.dispatch(
            request(Command::Remember(key()), 8),
            Duration::from_secs(2),
            false
        )
        .result,
        Err(DispatchError::Expired)
    ));
    let mut wrong = request(Command::Clear, 9);
    wrong.generation = Generation(NonZeroU64::new(2).unwrap());
    let reply = d.dispatch(wrong, Duration::ZERO, false);
    assert_eq!(reply.generation.0.get(), 2);
    assert!(matches!(reply.result, Err(DispatchError::WrongGeneration)));
    assert!(matches!(
        Command::Put(key(), Secret::new(vec![0; MAX_SECRET_BYTES + 1])).validate_size(),
        Err(DispatchError::TooLarge)
    ));
}
#[test]
fn explicit_failure_is_reported() {
    let mut d = Dispatcher::with_backend(
        generation(),
        Provider {
            fail: true,
            ..Provider::default()
        },
    );
    d.dispatch(
        request(Command::Put(key(), Secret::new(vec![1])), 1),
        Duration::ZERO,
        false,
    );
    assert!(matches!(
        d.dispatch(request(Command::Remember(key()), 2), Duration::ZERO, false)
            .result,
        Err(DispatchError::MutationUncertain(CredentialError::Backend))
    ));
    assert!(matches!(
        d.dispatch(request(Command::Forget(key()), 3), Duration::ZERO, false)
            .result,
        Err(DispatchError::MutationUncertain(CredentialError::Backend))
    ));
}

#[test]
fn mutations_can_take_effect_before_error_and_session_is_reconciled() {
    struct Mutates(Rc<RefCell<Option<Vec<u8>>>>);
    impl PersistentCredentials for Mutates {
        fn persist(&mut self, _: &CredentialKey, s: &Secret) -> Result<(), CredentialError> {
            *self.0.borrow_mut() = Some(s.expose().to_vec());
            Err(CredentialError::Backend)
        }
        fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
            Ok(self.0.borrow().clone().map(Secret::new))
        }
        fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
            *self.0.borrow_mut() = None;
            Err(CredentialError::Backend)
        }
    }
    let stored = Rc::new(RefCell::new(None));
    let mut d = Dispatcher::with_backend(generation(), Mutates(stored.clone()));
    d.dispatch(
        request(Command::Put(key(), Secret::new(vec![7])), 1),
        Duration::ZERO,
        false,
    );
    assert!(matches!(
        d.dispatch(request(Command::Remember(key()), 2), Duration::ZERO, false)
            .result,
        Err(DispatchError::MutationUncertain(CredentialError::Backend))
    ));
    assert_eq!(*stored.borrow(), Some(vec![7]));
    assert!(
        matches!(d.dispatch(request(Command::Get(key()), 3), Duration::ZERO, false).result, Ok(Response::Secret(Some(s))) if s.expose() == [7])
    );
    assert!(matches!(
        d.dispatch(request(Command::Forget(key()), 4), Duration::ZERO, false)
            .result,
        Err(DispatchError::MutationUncertain(CredentialError::Backend))
    ));
    assert!(stored.borrow().is_none());
    assert!(matches!(
        d.dispatch(request(Command::Get(key()), 5), Duration::ZERO, false)
            .result,
        Ok(Response::Secret(None))
    ));
}
