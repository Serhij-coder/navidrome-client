//! Input boundary for owned P01 HTTP fixtures, not production stream URLs.
use crate::Error;

/// Accept only an explicit numeric loopback HTTP authority and port. Query
/// credentials remain opaque and must never enter diagnostics. This validates
/// the initial URL; the trusted fixture must not redirect an mpv request.
pub fn validate_loopback_probe_url(source: &str) -> Result<(), Error> {
    if source.len() > 4096 || source.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(Error::InvalidInput);
    }
    let authority = source
        .strip_prefix("http://127.0.0.1:")
        .ok_or(Error::InvalidInput)?;
    let port = authority.split(['/', '?', '#']).next().unwrap_or("");
    if port.is_empty()
        || !port.bytes().all(|byte| byte.is_ascii_digit())
        || port.parse::<u16>().ok().is_none_or(|port| port == 0)
    {
        return Err(Error::InvalidInput);
    }
    let parsed = url::Url::parse(source).map_err(|_| Error::InvalidInput)?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("127.0.0.1")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/native-spike/probe_source.rs"]
mod tests;
