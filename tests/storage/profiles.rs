use feishin_storage::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Memory {
    bytes: Rc<RefCell<Option<Vec<u8>>>>,
    fail: Rc<RefCell<bool>>,
}
impl AtomicStore for Memory {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.bytes.borrow().clone())
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if *self.fail.borrow() {
            return Err(Error::Unavailable);
        }
        *self.bytes.borrow_mut() = Some(bytes.to_vec());
        Ok(())
    }
}
fn data() -> ProfileData {
    ProfileData {
        profiles: vec![Profile {
            id: "lan".into(),
            name: "Fixture".into(),
            endpoint: "http://192.0.2.4:4533/music".into(),
            backend: BackendKind::Navidrome,
        }],
        current_server: Some("lan".into()),
        preferences: Preferences {
            language: "uk".into(),
            follow_system_theme: true,
        },
    }
}
#[test]
fn defaults_and_partial_recovery_are_explicit_and_read_only() {
    assert_eq!(
        decode(include_bytes!("../fixtures/p02/storage/defaults.json"))
            .unwrap()
            .data,
        ProfileData::default()
    );
    let store = Memory::default();
    let original = include_bytes!("../fixtures/p02/storage/partial.json").to_vec();
    *store.bytes.borrow_mut() = Some(original.clone());
    let loaded = ProfileRepository::new(store.clone()).load().unwrap();
    assert_eq!(loaded.data.profiles.len(), 1);
    assert_eq!(loaded.data.current_server, None);
    assert_eq!(loaded.data.preferences.language, "en");
    assert!(loaded.data.preferences.follow_system_theme);
    assert_eq!(
        loaded.recovered,
        vec![
            Recovery::Profile(1),
            Recovery::DuplicateProfile(2),
            Recovery::Profile(3),
            Recovery::CurrentServer,
            Recovery::Language
        ]
    );
    assert_eq!(*store.bytes.borrow(), Some(original));
}
#[test]
fn failures_preserve_last_value_and_future_or_malformed_schema() {
    let store = Memory::default();
    let mut repo = ProfileRepository::new(store.clone());
    assert_eq!(repo.load().unwrap().data, ProfileData::default());
    repo.save(&data()).unwrap();
    let before = store.bytes.borrow().clone();
    *store.fail.borrow_mut() = true;
    assert_eq!(repo.save(&ProfileData::default()), Err(Error::Unavailable));
    assert_eq!(*store.bytes.borrow(), before);
    assert_eq!(repo.load().unwrap().data, data());
    *store.fail.borrow_mut() = false;
    for (fixture, error) in [
        (
            &include_bytes!("../fixtures/p02/storage/future.json")[..],
            Error::UnsupportedVersion,
        ),
        (
            &include_bytes!("../fixtures/p02/storage/malformed.json")[..],
            Error::InvalidDocument,
        ),
    ] {
        *store.bytes.borrow_mut() = Some(fixture.to_vec());
        assert_eq!(repo.load(), Err(error));
        assert_eq!(repo.save(&data()), Err(error));
        assert_eq!(*store.bytes.borrow(), Some(fixture.to_vec()));
    }
}
#[test]
fn secrets_invalid_schemes_and_unbounded_data_are_rejected() {
    let mut repo = ProfileRepository::new(Memory::default());
    for endpoint in [
        "file:///tmp/song",
        "https://u:p@example.test",
        "https://example.test?token=synthetic",
        "https://example.test#synthetic",
        "not a URL",
    ] {
        let mut value = data();
        value.profiles[0].endpoint = endpoint.into();
        assert_eq!(repo.save(&value), Err(Error::InvalidProfile));
    }
    let mut duplicate = data();
    duplicate.profiles.push(duplicate.profiles[0].clone());
    assert_eq!(repo.save(&duplicate), Err(Error::InvalidProfile));
    let mut orphan = data();
    orphan.current_server = Some("unknown".into());
    assert_eq!(repo.save(&orphan), Err(Error::InvalidProfile));
    assert_eq!(decode(&vec![b' '; MAX_BYTES + 1]), Err(Error::TooLarge));
    let bytes = serde_json::to_vec(&serde_json::json!({"schema_version":1,"data":{"profiles":vec![serde_json::Value::Null;MAX_PROFILES+1]}})).unwrap();
    assert_eq!(decode(&bytes), Err(Error::TooLarge));
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_roundtrip_replacement_isolated_and_interrupted_temp_ignored() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("store_authentication");
    std::fs::write(&original, "untouched upstream fixture").unwrap();
    let store = NativeStore::open(root.path().canonicalize().unwrap()).unwrap();
    let path = store.path();
    let mut repo = ProfileRepository::new(store);
    repo.save(&data()).unwrap();
    assert_eq!(repo.load().unwrap().data, data());
    std::fs::write(
        path.parent().unwrap().join(".interrupted-write"),
        b"partial json",
    )
    .unwrap();
    let mut reopened =
        ProfileRepository::new(NativeStore::open(root.path().canonicalize().unwrap()).unwrap());
    assert_eq!(reopened.load().unwrap().data, data());
    reopened.save(&ProfileData::default()).unwrap();
    assert_eq!(reopened.load().unwrap().data, ProfileData::default());
    assert_eq!(
        std::fs::read_to_string(original).unwrap(),
        "untouched upstream fixture"
    );
    std::fs::write(&path, include_bytes!("../fixtures/p02/storage/future.json")).unwrap();
    assert_eq!(reopened.save(&data()), Err(Error::UnsupportedVersion));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        include_bytes!("../fixtures/p02/storage/future.json")
    );
    std::fs::write(&path, vec![0; MAX_BYTES + 1]).unwrap();
    assert_eq!(reopened.load(), Err(Error::TooLarge));
    assert!(matches!(
        NativeStore::open("relative"),
        Err(Error::UnsafePath)
    ));
}
#[cfg(unix)]
#[test]
fn native_symlinks_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("feishin-rust-dev-p02")).unwrap();
    assert!(matches!(
        NativeStore::open(root.path().canonicalize().unwrap()),
        Err(Error::UnsafePath)
    ));
}

#[test]
fn recovery_field_shapes_and_unknown_keys_are_predictable() {
    let loaded = decode(br#"{"schema_version":1,"data":{"profiles":false,"current_server":12,"preferences":{"language":"uk","follow_system_theme":null}}}"#).unwrap();
    assert!(loaded.data.profiles.is_empty());
    assert_eq!(loaded.data.preferences.language, "uk");
    assert!(!loaded.data.preferences.follow_system_theme);
    assert_eq!(
        loaded.recovered,
        vec![
            Recovery::Profiles,
            Recovery::CurrentServer,
            Recovery::FollowSystemTheme
        ]
    );
    let loaded = decode(br#"{"schema_version":1,"data":{"preferences":false}}"#).unwrap();
    assert_eq!(loaded.data, ProfileData::default());
    assert_eq!(
        loaded.recovered,
        vec![Recovery::Language, Recovery::FollowSystemTheme]
    );
    for raw in [
        br#"{"schema_version":1,"data":{},"credential":"synthetic"}"#.as_slice(),
        br#"{"schema_version":1,"data":{"preferences":{"unknown":true}}}"#.as_slice(),
        br#"{"schema_version":1,"data":{"credential":"synthetic"}}"#.as_slice(),
        br#"{"schema_version":1}"#.as_slice(),
    ] {
        assert_eq!(decode(raw), Err(Error::InvalidDocument));
    }
    let mut oversized = data();
    oversized.profiles = (0..MAX_PROFILES)
        .map(|i| Profile {
            id: i.to_string(),
            name: "x".repeat(256),
            endpoint: format!("https://example.test/{}", "x".repeat(4000)),
            backend: BackendKind::Navidrome,
        })
        .collect();
    oversized.current_server = None;
    assert_eq!(
        ProfileRepository::new(Memory::default()).save(&oversized),
        Err(Error::TooLarge)
    );
}
