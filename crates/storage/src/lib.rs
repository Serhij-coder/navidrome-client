//! Bounded non-secret development profiles. Native methods are blocking worker APIs.
//! No shell loads these profiles yet. Credentials and historical imports are separate work.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod imports;
pub mod journal;
pub mod settings_migration;
pub mod state;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserStore;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeStore;

pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_BYTES: usize = 256 * 1024;
pub const MAX_PROFILES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unavailable,
    Io,
    TooLarge,
    InvalidDocument,
    UnsupportedVersion,
    InvalidProfile,
    UnsafePath,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "profile storage: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    Navidrome,
    Subsonic,
    Jellyfin,
}

/// An endpoint cannot contain URL credentials, query tokens or fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub backend: BackendKind,
}
impl Profile {
    /// Convert the durable string DTO identity at the application boundary.
    pub fn server_id(&self) -> feishin_domain::ServerId {
        feishin_domain::ServerId::new(&self.id)
    }
    pub fn server_type(&self) -> feishin_domain::ServerType {
        match self.backend {
            BackendKind::Navidrome => feishin_domain::ServerType::Navidrome,
            BackendKind::Subsonic => feishin_domain::ServerType::Subsonic,
            BackendKind::Jellyfin => feishin_domain::ServerType::Jellyfin,
        }
    }
    fn valid(&self) -> bool {
        let Ok(url) = url::Url::parse(&self.endpoint) else {
            return false;
        };
        !self.id.trim().is_empty()
            && self.id.len() <= 128
            && !self.name.trim().is_empty()
            && self.name.len() <= 256
            && self.endpoint.len() <= 4096
            && matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub language: String,
    pub follow_system_theme: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: "en".into(),
            follow_system_theme: false,
        }
    }
}
impl Preferences {
    fn valid(&self) -> bool {
        !self.language.is_empty()
            && self.language.len() <= 64
            && self
                .language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileData {
    pub profiles: Vec<Profile>,
    pub current_server: Option<String>,
    pub preferences: Preferences,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    Profiles,
    Profile(usize),
    DuplicateProfile(usize),
    CurrentServer,
    Language,
    FollowSystemTheme,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub data: ProfileData,
    pub recovered: Vec<Recovery>,
}

/// A single atomic value. Errors must leave the prior value intact or report Io
/// when durability is uncertain. Implementations must enforce MAX_BYTES on reads.
/// Access is serialized through the repository; cross-process writers are unsupported.
pub trait AtomicStore {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error>;
    fn replace(&mut self, bytes: &[u8]) -> Result<(), Error>;
}

pub struct ProfileRepository<S> {
    store: S,
}
impl<S: AtomicStore> ProfileRepository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    pub fn load(&mut self) -> Result<Loaded, Error> {
        match self.store.read()? {
            None => Ok(Loaded {
                data: ProfileData::default(),
                recovered: vec![],
            }),
            Some(bytes) => decode(&bytes),
        }
    }
    /// Explicit user/application save; load recovery never writes automatically.
    /// Future schemas and malformed whole documents are protected from overwrite.
    pub fn save(&mut self, data: &ProfileData) -> Result<(), Error> {
        if data.profiles.len() > MAX_PROFILES || !data.preferences.valid() {
            return Err(Error::InvalidProfile);
        }
        let mut ids = std::collections::HashSet::new();
        if data
            .profiles
            .iter()
            .any(|p| !p.valid() || !ids.insert(&p.id))
            || data
                .current_server
                .as_ref()
                .is_some_and(|id| !ids.contains(id))
        {
            return Err(Error::InvalidProfile);
        }
        if let Some(old) = self.store.read()? {
            decode(&old)?;
        }
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": SCHEMA_VERSION, "data": data
        }))
        .map_err(|_| Error::InvalidDocument)?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        self.store.replace(&bytes)
    }
}

pub fn decode(bytes: &[u8]) -> Result<Loaded, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::InvalidDocument)?;
    let root = value.as_object().ok_or(Error::InvalidDocument)?;
    let version = root
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or(Error::InvalidDocument)?;
    if version != SCHEMA_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    if root
        .keys()
        .any(|key| key != "schema_version" && key != "data")
    {
        return Err(Error::InvalidDocument);
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or(Error::InvalidDocument)?;
    if data
        .keys()
        .any(|key| !matches!(key.as_str(), "profiles" | "current_server" | "preferences"))
    {
        return Err(Error::InvalidDocument);
    }
    let mut loaded = Loaded {
        data: ProfileData::default(),
        recovered: vec![],
    };
    if let Some(profiles) = data.get("profiles") {
        if let Some(profiles) = profiles.as_array() {
            if profiles.len() > MAX_PROFILES {
                return Err(Error::TooLarge);
            }
            for (i, raw) in profiles.iter().enumerate() {
                match serde_json::from_value::<Profile>(raw.clone()) {
                    Ok(profile) if profile.valid() => {
                        if loaded.data.profiles.iter().any(|p| p.id == profile.id) {
                            loaded.recovered.push(Recovery::DuplicateProfile(i));
                        } else {
                            loaded.data.profiles.push(profile);
                        }
                    }
                    _ => loaded.recovered.push(Recovery::Profile(i)),
                }
            }
        } else {
            loaded.recovered.push(Recovery::Profiles);
        }
    }
    if let Some(current) = data.get("current_server")
        && !current.is_null()
    {
        match current.as_str() {
            Some(id) if loaded.data.profiles.iter().any(|p| p.id == id) => {
                loaded.data.current_server = Some(id.into())
            }
            _ => loaded.recovered.push(Recovery::CurrentServer),
        }
    }
    if let Some(preferences) = data.get("preferences") {
        if let Some(prefs) = preferences.as_object() {
            if prefs
                .keys()
                .any(|key| !matches!(key.as_str(), "language" | "follow_system_theme"))
            {
                return Err(Error::InvalidDocument);
            }
            if let Some(language) = prefs.get("language") {
                let candidate = Preferences {
                    language: language.as_str().unwrap_or("").into(),
                    ..Preferences::default()
                };
                if candidate.valid() {
                    loaded.data.preferences.language = candidate.language;
                } else {
                    loaded.recovered.push(Recovery::Language);
                }
            }
            if let Some(follow) = prefs.get("follow_system_theme") {
                if let Some(follow) = follow.as_bool() {
                    loaded.data.preferences.follow_system_theme = follow;
                } else {
                    loaded.recovered.push(Recovery::FollowSystemTheme);
                }
            }
        } else {
            loaded
                .recovered
                .extend([Recovery::Language, Recovery::FollowSystemTheme]);
        }
    }
    Ok(loaded)
}
