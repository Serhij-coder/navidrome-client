#![cfg(target_os = "linux")]
use feishin_storage::linux_credentials::LinuxCredentials;
#[test]
fn constructor_is_lazy() {
    let _ = LinuxCredentials::new();
}

use feishin_domain::Secret;
use feishin_storage::{
    BackendKind, Profile,
    credentials::{CredentialKey, CredentialSlot, PersistentCredentials},
};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start_bus(root: &std::path::Path) -> (OwnedChild, String) {
    let config = root.join("private-bus.conf");
    std::fs::write(&config, r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd"><busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>"#).unwrap();
    let mut bus = OwnedChild(
        Command::new("dbus-daemon")
            .arg("--nofork")
            .arg("--print-address=1")
            .arg("--config-file")
            .arg(config)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = bus.0.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut address = String::new();
        let result = BufReader::new(stdout).read_line(&mut address);
        let _ = sender.send(result.map(|_| address));
    });
    let address = receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert!(!address.trim().is_empty());
    (bus, address.trim().to_owned())
}
fn start_daemon(
    address: &str,
    data: &std::path::Path,
    config: &std::path::Path,
    cache: &std::path::Path,
    runtime: &std::path::Path,
) -> OwnedChild {
    let mut child = OwnedChild(
        Command::new("gnome-keyring-daemon")
            .args([
                "--foreground",
                "--components=secrets",
                "--unlock",
                "--control-directory",
            ])
            .arg(runtime.join("keyring"))
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .env("XDG_DATA_HOME", data)
            .env("XDG_CONFIG_HOME", config)
            .env("XDG_CACHE_HOME", cache)
            .env("XDG_RUNTIME_DIR", runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    child
        .0
        .stdin
        .take()
        .unwrap()
        .write_all(b"synthetic-unlock-passphrase\n")
        .unwrap();
    child
}
fn wait_ready(adapter: &mut LinuxCredentials, key: &CredentialKey) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if adapter.load(key).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "private provider did not become ready"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn assert_no_plaintext(path: &std::path::Path, needle: &[u8]) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            assert_no_plaintext(&entry.path(), needle);
        } else if ty.is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(!bytes.windows(needle.len()).any(|w| w == needle));
        }
    }
}
#[test]
#[ignore = "requires an explicitly owned GNOME Secret Service environment"]
fn owned_secret_service_roundtrip() {
    assert_eq!(
        std::env::var("P02_OWNED_SECRET_SERVICE").as_deref(),
        Ok("1")
    );
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let config = root.path().join("config");
    let runtime = root.path().join("runtime");
    let cache = root.path().join("cache");
    for dir in [&data, &config, &cache, &runtime] {
        std::fs::create_dir(dir).unwrap();
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (_bus, address) = start_bus(root.path());
    let address = address.as_str();
    let mut daemon = start_daemon(address, &data, &config, &cache, &runtime);
    let connection = zbus::blocking::connection::Builder::address(address)
        .unwrap()
        .method_timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let p = Profile {
        id: "synthetic".into(),
        name: "test".into(),
        endpoint: "https://example.test/".into(),
        backend: BackendKind::Navidrome,
    };
    let a = CredentialKey::new("owned-test", &p, "one", CredentialSlot::LoginPassword).unwrap();
    let b = CredentialKey::new("owned-test", &p, "two", CredentialSlot::LoginPassword).unwrap();
    let mut adapter = LinuxCredentials::with_connection(connection.clone());
    wait_ready(&mut adapter, &b);
    adapter
        .persist(&a, &Secret::new(b"first-synthetic-credential".to_vec()))
        .unwrap();
    assert_no_plaintext(root.path(), b"first-synthetic-credential");
    daemon.0.kill().unwrap();
    daemon.0.wait().unwrap();
    daemon = start_daemon(address, &data, &config, &cache, &runtime);
    wait_ready(&mut adapter, &b);
    assert!(adapter.load(&b).unwrap().is_none());
    assert_eq!(
        adapter.load(&a).unwrap().unwrap().expose(),
        b"first-synthetic-credential"
    );
    adapter
        .persist(&a, &Secret::new(b"second-synthetic-credential".to_vec()))
        .unwrap();
    assert_eq!(
        adapter.load(&a).unwrap().unwrap().expose(),
        b"second-synthetic-credential"
    );
    assert_no_plaintext(root.path(), b"second-synthetic-credential");
    {
        let service = secret_service::blocking::SecretService::connect_with_existing(
            secret_service::EncryptionType::Dh,
            connection.clone(),
        )
        .unwrap();
        service.get_default_collection().unwrap().lock().unwrap();
    }
    use feishin_storage::credentials::CredentialError;
    assert_eq!(adapter.load(&a).err(), Some(CredentialError::Locked));
    assert_eq!(
        adapter.persist(&a, &Secret::new(vec![1])),
        Err(CredentialError::Locked)
    );
    assert_eq!(adapter.delete(&a), Err(CredentialError::Locked));
    daemon.0.kill().unwrap();
    daemon.0.wait().unwrap();
    let _daemon = start_daemon(address, &data, &config, &cache, &runtime);
    wait_ready(&mut adapter, &b);
    adapter.delete(&a).unwrap();
    assert!(adapter.load(&a).unwrap().is_none());
    adapter.delete(&a).unwrap();
    // Exercise duplicate handling and attribute isolation against real provider items.
    {
        use std::collections::HashMap;
        let service = secret_service::blocking::SecretService::connect_with_existing(
            secret_service::EncryptionType::Dh,
            connection.clone(),
        )
        .unwrap();
        let collection = service.get_default_collection().unwrap();
        let nondefault = CredentialKey::new(
            "owned-test",
            &p,
            "nondefault",
            CredentialSlot::LoginPassword,
        )
        .unwrap();
        let digest = nondefault.attribute();
        let nondefault_attrs = HashMap::from([
            ("application", "feishin-rust"),
            ("scope", "development"),
            ("schema", "credential-v1"),
            ("key", digest.as_str()),
        ]);
        let session_collection = service.get_collection_by_alias("session").unwrap();
        session_collection
            .create_item(
                "test",
                nondefault_attrs.clone(),
                b"nondefault-original",
                false,
                "application/octet-stream",
            )
            .unwrap();
        assert_eq!(
            adapter.load(&nondefault).unwrap().unwrap().expose(),
            b"nondefault-original"
        );
        adapter
            .persist(&nondefault, &Secret::new(b"nondefault-updated".to_vec()))
            .unwrap();
        let found = service.search_items(nondefault_attrs.clone()).unwrap();
        assert_eq!(found.locked.len(), 0);
        assert_eq!(found.unlocked.len(), 1);
        assert_eq!(
            found.unlocked[0].get_secret().unwrap(),
            b"nondefault-updated"
        );
        adapter.delete(&nondefault).unwrap();
        assert!(adapter.load(&nondefault).unwrap().is_none());
        assert!(
            session_collection
                .search_items(nondefault_attrs)
                .unwrap()
                .is_empty()
        );
        let duplicate =
            CredentialKey::new("owned-test", &p, "duplicate", CredentialSlot::LoginPassword)
                .unwrap();
        let digest = duplicate.attribute();
        let own = HashMap::from([
            ("application", "feishin-rust"),
            ("scope", "development"),
            ("schema", "credential-v1"),
            ("key", digest.as_str()),
        ]);
        collection
            .create_item(
                "test",
                own.clone(),
                b"duplicate-one",
                false,
                "application/octet-stream",
            )
            .unwrap();
        session_collection
            .create_item(
                "test",
                own.clone(),
                b"duplicate-two",
                false,
                "application/octet-stream",
            )
            .unwrap();
        assert_eq!(
            adapter.load(&duplicate).err(),
            Some(CredentialError::Ambiguous)
        );
        assert_eq!(
            adapter.persist(&duplicate, &Secret::new(b"replacement".to_vec())),
            Err(CredentialError::Ambiguous)
        );
        assert_eq!(adapter.delete(&duplicate), Err(CredentialError::Ambiguous));
        let mut values: Vec<_> = service
            .search_items(own)
            .unwrap()
            .unlocked
            .iter()
            .map(|item| item.get_secret().unwrap())
            .collect();
        values.sort();
        assert_eq!(
            values,
            vec![b"duplicate-one".to_vec(), b"duplicate-two".to_vec()]
        );
        let digest = b.attribute();
        let foreign_app = HashMap::from([
            ("application", "other-app"),
            ("scope", "development"),
            ("schema", "credential-v1"),
            ("key", digest.as_str()),
        ]);
        let foreign_scope = HashMap::from([
            ("application", "feishin-rust"),
            ("scope", "other-scope"),
            ("schema", "credential-v1"),
            ("key", digest.as_str()),
        ]);
        collection
            .create_item(
                "test",
                foreign_app.clone(),
                b"foreign-app",
                false,
                "application/octet-stream",
            )
            .unwrap();
        collection
            .create_item(
                "test",
                foreign_scope.clone(),
                b"foreign-scope",
                false,
                "application/octet-stream",
            )
            .unwrap();
        assert!(adapter.load(&b).unwrap().is_none());
        adapter.delete(&b).unwrap();
        for (attributes, expected) in [
            (foreign_app, b"foreign-app".as_slice()),
            (foreign_scope, b"foreign-scope".as_slice()),
        ] {
            let items = collection.search_items(attributes).unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].get_secret().unwrap(), expected);
        }
    }
    assert_no_plaintext(root.path(), b"first-synthetic-credential");
    assert_no_plaintext(root.path(), b"second-synthetic-credential");
}

#[test]
fn private_bus_without_provider_is_unavailable() {
    let root = tempfile::tempdir().unwrap();
    let (_bus, address) = start_bus(root.path());
    let connection = zbus::blocking::connection::Builder::address(address.trim())
        .unwrap()
        .method_timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let profile = Profile {
        id: "id".into(),
        name: "name".into(),
        endpoint: "https://example.test".into(),
        backend: BackendKind::Subsonic,
    };
    let key =
        CredentialKey::new("dev", &profile, "account", CredentialSlot::LoginPassword).unwrap();
    let mut adapter = LinuxCredentials::with_connection(connection);
    assert_eq!(
        adapter.load(&key).err(),
        Some(feishin_storage::credentials::CredentialError::Unavailable)
    );
}
