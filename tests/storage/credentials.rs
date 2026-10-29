use feishin_domain::Secret;
use feishin_storage::{BackendKind, Profile, credentials::*};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
fn profile() -> Profile {
    Profile {
        id: "server".into(),
        name: "name".into(),
        endpoint: "https://example.test/".into(),
        backend: BackendKind::Navidrome,
    }
}
fn key(n: &str) -> CredentialKey {
    CredentialKey::new("dev", &profile(), n, CredentialSlot::LoginPassword).unwrap()
}
#[derive(Default)]
struct Fake {
    calls: Rc<RefCell<usize>>,
    values: HashMap<CredentialKey, Vec<u8>>,
    fail: bool,
}
impl PersistentCredentials for Fake {
    fn persist(&mut self, k: &CredentialKey, s: &Secret) -> Result<(), CredentialError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CredentialError::Unavailable);
        }
        self.values.insert(k.clone(), s.expose().to_vec());
        Ok(())
    }
    fn load(&mut self, k: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
        *self.calls.borrow_mut() += 1;
        Ok(self.values.get(k).map(|v| Secret::new(v.clone())))
    }
    fn delete(&mut self, k: &CredentialKey) -> Result<(), CredentialError> {
        *self.calls.borrow_mut() += 1;
        if self.fail {
            return Err(CredentialError::Unavailable);
        }
        self.values.remove(k);
        Ok(())
    }
}
#[test]
fn identity_and_validation() {
    let p = profile();
    let base = key("account");
    let mut p2 = p.clone();
    p2.name = "other".into();
    assert_eq!(
        base,
        CredentialKey::new("dev", &p2, "account", CredentialSlot::LoginPassword).unwrap()
    );
    for changed in [
        {
            let mut v = p.clone();
            v.id = "other".into();
            v
        },
        {
            let mut v = p.clone();
            v.endpoint = "https://other.test/".into();
            v
        },
        {
            let mut v = p.clone();
            v.backend = BackendKind::Jellyfin;
            v
        },
    ] {
        assert_ne!(
            base,
            CredentialKey::new("dev", &changed, "account", CredentialSlot::LoginPassword).unwrap()
        );
    }
    assert_ne!(
        base,
        CredentialKey::new("other", &p, "account", CredentialSlot::LoginPassword).unwrap()
    );
    assert_ne!(base, key("other"));
    assert_ne!(
        base,
        CredentialKey::new("dev", &p, "account", CredentialSlot::SessionTokens).unwrap()
    );
    assert!(CredentialKey::new("", &p, "account", CredentialSlot::LoginPassword).is_err());
    assert!(
        CredentialKey::new("dev", &p, &"x".repeat(257), CredentialSlot::LoginPassword).is_err()
    );
    assert!(!format!("{base:?}").contains("account"));
}
#[test]
fn explicit_persistence_and_failure() {
    let calls = Rc::new(RefCell::new(0));
    let mut owner = CredentialOwner::with_backend(Fake {
        calls: calls.clone(),
        ..Default::default()
    });
    let k = key("a");
    owner
        .put_session(k.clone(), Secret::new(b"secret".to_vec()))
        .unwrap();
    assert_eq!(*calls.borrow(), 0);
    owner.remember(&k).unwrap();
    owner.clear_session();
    assert!(owner.restore_remembered(&k).unwrap());
    assert_eq!(owner.session(&k).unwrap().expose(), b"secret");
    owner.forget(&k).unwrap();
    assert!(!owner.restore_remembered(&k).unwrap());
    let mut failed = CredentialOwner::with_backend(Fake {
        fail: true,
        ..Default::default()
    });
    failed.put_session(k.clone(), Secret::new(vec![1])).unwrap();
    assert_eq!(failed.remember(&k), Err(CredentialError::Unavailable));
    assert!(failed.session(&k).is_some());
    assert_eq!(failed.forget(&k), Err(CredentialError::Unavailable));
    assert!(failed.session(&k).is_none());
    let mut unsupported = CredentialOwner::new();
    unsupported
        .put_session(k.clone(), Secret::new(vec![1]))
        .unwrap();
    assert_eq!(unsupported.remember(&k), Err(CredentialError::Unsupported));
    assert_eq!(
        unsupported.restore_remembered(&k),
        Err(CredentialError::Unsupported)
    );
}
#[test]
fn bounds_preserve_existing() {
    let mut owner = CredentialOwner::new();
    let k = key("0");
    owner.put_session(k.clone(), Secret::new(vec![1])).unwrap();
    assert_eq!(
        owner.put_session(k.clone(), Secret::new(vec![2; MAX_SECRET_BYTES + 1])),
        Err(CredentialError::TooLarge)
    );
    assert_eq!(owner.session(&k).unwrap().expose(), &[1]);
    for i in 1..MAX_SESSION_ENTRIES {
        owner
            .put_session(key(&i.to_string()), Secret::new(vec![]))
            .unwrap();
    }
    assert_eq!(
        owner.put_session(key("extra"), Secret::new(vec![])),
        Err(CredentialError::Capacity)
    );
    owner.put_session(k.clone(), Secret::new(vec![3])).unwrap();
    assert_eq!(owner.session(&k).unwrap().expose(), &[3]);
}

#[test]
fn oversized_restore_preserves_session() {
    let k = key("restore");
    let mut fake = Fake::default();
    fake.values.insert(k.clone(), vec![7; MAX_SECRET_BYTES + 1]);
    let mut owner = CredentialOwner::with_backend(fake);
    owner.put_session(k.clone(), Secret::new(vec![1])).unwrap();
    assert_eq!(owner.restore_remembered(&k), Err(CredentialError::TooLarge));
    assert_eq!(owner.session(&k).unwrap().expose(), &[1]);
}
#[test]
fn canonical_endpoint_and_input_limits() {
    let p = profile();
    let base = CredentialKey::new("dev", &p, "account", CredentialSlot::LoginPassword).unwrap();
    let mut canonical = p.clone();
    canonical.endpoint = "HTTPS://EXAMPLE.TEST:443".into();
    assert_eq!(
        base,
        CredentialKey::new("dev", &canonical, "account", CredentialSlot::LoginPassword).unwrap()
    );
    for endpoint in [
        "https://user:pass@example.test",
        "https://example.test/?token=x",
        "https://example.test/#x",
        "file:///tmp/a",
        "invalid",
    ] {
        let mut invalid = p.clone();
        invalid.endpoint = endpoint.into();
        assert_eq!(
            CredentialKey::new("dev", &invalid, "account", CredentialSlot::LoginPassword),
            Err(CredentialError::InvalidKey)
        );
    }
    assert!(
        CredentialKey::new(
            &"n".repeat(128),
            &p,
            &"a".repeat(256),
            CredentialSlot::LoginPassword
        )
        .is_ok()
    );
    assert_eq!(
        CredentialKey::new(&"n".repeat(129), &p, "a", CredentialSlot::LoginPassword),
        Err(CredentialError::InvalidKey)
    );
    assert_eq!(
        CredentialKey::new("dev", &p, " ", CredentialSlot::LoginPassword),
        Err(CredentialError::InvalidKey)
    );
}
