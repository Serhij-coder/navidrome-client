//! Browser credential policy fixture. Values remain in the local owner's memory.
use feishin_domain::Secret;
use feishin_storage::{BackendKind, Profile, credentials::*};

pub fn run(action: &str) -> Result<bool, CredentialError> {
    let profile = Profile {
        id: "fixture-server".into(),
        name: "Fixture".into(),
        endpoint: "http://192.0.2.1/music".into(),
        backend: BackendKind::Navidrome,
    };
    let key = CredentialKey::new(
        "browser-fixture",
        &profile,
        "fixture-account",
        CredentialSlot::SessionTokens,
    )?;
    let mut owner = CredentialOwner::new();
    match action {
        "credential-default" => Ok(owner.session(&key).is_none()),
        "credential-flow" => {
            owner.put_session(
                key.clone(),
                Secret::new(b"synthetic-session-only-value".to_vec()),
            )?;
            let unsupported = owner.remember(&key) == Err(CredentialError::Unsupported)
                && owner.restore_remembered(&key) == Err(CredentialError::Unsupported);
            let retained = owner
                .session(&key)
                .is_some_and(|s| s.expose() == b"synthetic-session-only-value");
            let other = CredentialKey::new(
                "another-namespace",
                &profile,
                "fixture-account",
                CredentialSlot::SessionTokens,
            )?;
            let isolated = owner.session(&other).is_none() && other != key;
            let rejected = owner
                .put_session(key.clone(), Secret::new(vec![0; MAX_SECRET_BYTES + 1]))
                == Err(CredentialError::TooLarge);
            let unchanged = owner
                .session(&key)
                .is_some_and(|s| s.expose() == b"synthetic-session-only-value");
            owner.forget(&key)?;
            let forgotten = owner.session(&key).is_none();
            owner.put_session(
                key.clone(),
                Secret::new(b"another-synthetic-value".to_vec()),
            )?;
            owner.clear_session();
            Ok(unsupported
                && retained
                && isolated
                && rejected
                && unchanged
                && forgotten
                && owner.session(&key).is_none()
                && key.attribute().len() == 64)
        }
        _ => Err(CredentialError::InvalidKey),
    }
}
