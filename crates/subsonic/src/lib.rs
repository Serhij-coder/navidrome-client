//! Portable Subsonic request construction. This crate performs no network I/O.
use md5::{Digest, Md5};
use std::fmt;
use url::Url;

const MAX_INPUT: usize = 8192;
const MAX_PAIRS: usize = 128;
const MAX_URL: usize = 65536;
const ALPHABET: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Closed errors never contain caller input or entropy provider details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidBase,
    InvalidEndpoint,
    InvalidQuery,
    InputTooLarge,
    EntropyUnavailable,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidBase => "invalid server base URL",
            Self::InvalidEndpoint => "invalid endpoint",
            Self::InvalidQuery => "invalid query",
            Self::InputTooLarge => "input exceeds limit",
            Self::EntropyUnavailable => "entropy unavailable",
        })
    }
}
impl std::error::Error for Error {}

/// A trusted caller supplies cryptographically strong entropy and fills every requested byte.
/// Provider errors are never retained. The boundary does not require `Send`.
pub trait Entropy {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), EntropyError>;
}
#[derive(Clone, Copy, Debug)]
pub struct EntropyError;

/// Authentication material is deliberately opaque and is not serializable.
pub struct Auth {
    username: String,
    mode: Mode,
}
enum Mode {
    Salted { salt: String, token: String },
    Legacy { password: String },
}
impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Auth([REDACTED])")
    }
}
impl Auth {
    /// Creates reusable salted credentials using a fresh, unbiased 12-character salt.
    pub fn salted(
        username: &str,
        password: &str,
        entropy: &mut dyn Entropy,
    ) -> Result<Self, Error> {
        check_auth_input(username, password)?;
        let mut salt = String::with_capacity(12);
        // Limit draws even if the provider repeatedly returns rejected bytes.
        for _ in 0..256 {
            let mut byte = [0];
            entropy
                .fill(&mut byte)
                .map_err(|_| Error::EntropyUnavailable)?;
            if byte[0] < 248 {
                salt.push(char::from(ALPHABET[usize::from(byte[0] % 62)]));
                if salt.len() == 12 {
                    break;
                }
            }
        }
        if salt.len() != 12 {
            return Err(Error::EntropyUnavailable);
        }
        let mut md5 = Md5::new();
        md5.update(password.as_bytes());
        md5.update(salt.as_bytes());
        let token = format!("{:x}", md5.finalize());
        Ok(Self {
            username: username.into(),
            mode: Mode::Salted { salt, token },
        })
    }
    /// Explicit opt-in for servers requiring the legacy plaintext password parameter.
    pub fn legacy(username: &str, password: &str) -> Result<Self, Error> {
        check_auth_input(username, password)?;
        Ok(Self {
            username: username.into(),
            mode: Mode::Legacy {
                password: password.into(),
            },
        })
    }
}
fn check_auth_input(username: &str, password: &str) -> Result<(), Error> {
    if username.len() > MAX_INPUT || password.len() > MAX_INPUT {
        Err(Error::InputTooLarge)
    } else {
        Ok(())
    }
}

/// A validated server base, with one trailing slash removed before adding `/rest`.
pub struct ServerBase(String);
impl fmt::Debug for ServerBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ServerBase([REDACTED])")
    }
}
impl ServerBase {
    pub fn parse(input: &str) -> Result<Self, Error> {
        if input.len() > MAX_INPUT {
            return Err(Error::InputTooLarge);
        }
        if input.chars().any(char::is_control) {
            return Err(Error::InvalidBase);
        }
        let input = input.trim();
        let authority = input
            .split_once("://")
            .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(""));
        if authority.is_none_or(|part| part.contains('@')) || !valid_percent_escapes(input) {
            return Err(Error::InvalidBase);
        }
        let url = Url::parse(input).map_err(|_| Error::InvalidBase)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || input.contains('\\')
        {
            return Err(Error::InvalidBase);
        }
        let mut base = url.to_string();
        if base.ends_with('/') {
            base.pop();
        }
        Ok(Self(base))
    }
    /// Construct a GET URL from ordered decoded query pairs; no parsing of route strings occurs.
    pub fn request(
        &self,
        auth: &Auth,
        endpoint: &str,
        pairs: &[(&str, &str)],
    ) -> Result<RequestUrl, Error> {
        if endpoint.is_empty()
            || endpoint.len() > MAX_INPUT
            || endpoint == "."
            || endpoint == ".."
            || !endpoint
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(Error::InvalidEndpoint);
        }
        if pairs.len() > MAX_PAIRS {
            return Err(Error::InputTooLarge);
        }
        let mut out = format!("{}/rest/{}?", self.0, endpoint);
        add_pair(&mut out, "c", "Feishin");
        add_pair(&mut out, "f", "json");
        add_pair(&mut out, "v", "1.13.0");
        add_pair(&mut out, "u", &auth.username);
        match &auth.mode {
            Mode::Salted { salt, token } => {
                add_pair(&mut out, "s", salt);
                add_pair(&mut out, "t", token);
            }
            Mode::Legacy { password } => add_pair(&mut out, "p", password),
        }
        for &(key, value) in pairs {
            if key.len() > MAX_INPUT || value.len() > MAX_INPUT {
                return Err(Error::InputTooLarge);
            }
            if key.is_empty() || reserved(key) {
                return Err(Error::InvalidQuery);
            }
            add_pair(&mut out, key, value);
            if out.len() > MAX_URL {
                return Err(Error::InputTooLarge);
            }
        }
        if out.len() > MAX_URL {
            return Err(Error::InputTooLarge);
        }
        Ok(RequestUrl(out))
    }
}
fn valid_percent_escapes(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}
fn reserved(key: &str) -> bool {
    matches!(
        key.split('[')
            .next()
            .unwrap_or(key)
            .to_ascii_lowercase()
            .as_str(),
        "c" | "f"
            | "v"
            | "u"
            | "p"
            | "s"
            | "t"
            | "username"
            | "password"
            | "token"
            | "salt"
            | "authorization"
            | "auth"
            | "apikey"
            | "api_key"
    )
}
fn add_pair(out: &mut String, key: &str, value: &str) {
    if !out.ends_with('?') {
        out.push('&');
    }
    encode(out, key);
    out.push('=');
    encode(out, value);
}
fn encode(out: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 15)]));
        }
    }
}
/// Opaque authenticated URL; expose only at the transport boundary.
pub struct RequestUrl(String);
impl RequestUrl {
    pub fn expose_for_transport(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for RequestUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RequestUrl([REDACTED])")
    }
}
