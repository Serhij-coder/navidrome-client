//! Bounded protocol discovery and a cooperative local transport boundary.
use crate::{Auth, Error, ServerBase};
use feishin_domain::{Clock, DomainError, LocalFuture, OperationContext, RequestContext};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    Generic,
    MissingParameter,
    IncompatibleClient,
    IncompatibleServer,
    Authentication,
    Forbidden,
    TrialExpired,
    DataNotFound,
    Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    Network,
    Timeout,
    Cancelled,
    Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpError {
    Authentication,
    Forbidden,
    NotFound,
    Client,
    Server,
    Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExchangeError {
    Context(DomainError),
    Stale,
    Transport(TransportError),
    Http(HttpError),
    Malformed,
    Protocol(ProtocolError),
    Unsupported,
    Request(Error),
}

/// Only metadata needed for discovery is retained; unknown extension names are ignored.
#[derive(Clone, Eq, PartialEq)]
pub struct Discovery {
    pub protocol_version: String,
    pub server_version: Option<String>,
    extensions: BTreeMap<String, Vec<u64>>,
    source_base: String,
}
impl fmt::Debug for Discovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Discovery([REDACTED])")
    }
}
impl Discovery {
    pub fn supports(&self, name: &str, version: u64) -> bool {
        self.extensions
            .get(name)
            .is_some_and(|v| v.contains(&version))
    }
    pub fn versions(&self, name: &str) -> Option<&[u64]> {
        self.extensions.get(name).map(Vec::as_slice)
    }
}
const KNOWN: &[&str] = &[
    "formPost",
    "indexBasedQueue",
    "playbackReport",
    "songLyrics",
    "topSongsByArtistId",
    "transcodeOffset",
    "transcoding",
];
fn envelope(bytes: &[u8]) -> Result<Map<String, Value>, ExchangeError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ExchangeError::Malformed)?;
    let Value::Object(mut root) = value else {
        return Err(ExchangeError::Malformed);
    };
    let Some(Value::Object(response)) = root.remove("subsonic-response") else {
        return Err(ExchangeError::Malformed);
    };
    if response
        .get("version")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(ExchangeError::Malformed);
    }
    match response.get("status").and_then(Value::as_str) {
        Some("ok") => Ok(response),
        Some("failed") => {
            let code = response
                .get("error")
                .and_then(Value::as_object)
                .and_then(|e| e.get("code"))
                .and_then(|v| {
                    v.as_i64()
                        .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                })
                .ok_or(ExchangeError::Malformed)?;
            let category = match code {
                0 => ProtocolError::Generic,
                10 => ProtocolError::MissingParameter,
                20 => ProtocolError::IncompatibleClient,
                30 => ProtocolError::IncompatibleServer,
                40 => ProtocolError::Authentication,
                50 => ProtocolError::Forbidden,
                60 => ProtocolError::TrialExpired,
                70 => ProtocolError::DataNotFound,
                _ => ProtocolError::Other,
            };
            Err(ExchangeError::Protocol(category))
        }
        _ => Err(ExchangeError::Malformed),
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostPreference {
    Get,
    PreferForm,
    RequireForm,
}
pub enum Method {
    Get,
    FormPost,
}
/// Authenticated request is opaque in Debug; expose only to an authorized transport.
pub struct PlannedRequest {
    url: String,
    body: Option<String>,
}
impl fmt::Debug for PlannedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PlannedRequest([REDACTED])")
    }
}
impl PlannedRequest {
    pub fn method(&self) -> Method {
        if self.body.is_some() {
            Method::FormPost
        } else {
            Method::Get
        }
    }
    pub fn expose_url_for_transport(&self) -> &str {
        &self.url
    }
    pub fn expose_body_for_transport(&self) -> Option<&str> {
        self.body.as_deref()
    }
    pub fn content_type(&self) -> Option<&'static str> {
        self.body
            .as_ref()
            .map(|_| "application/x-www-form-urlencoded")
    }
    fn size(&self) -> usize {
        self.url
            .len()
            .saturating_add(self.body.as_ref().map_or(0, String::len))
    }
}
pub fn plan_request(
    base: &ServerBase,
    auth: &Auth,
    endpoint: &str,
    pairs: &[(&str, &str)],
    preference: PostPreference,
    discovery: Option<&Discovery>,
) -> Result<PlannedRequest, ExchangeError> {
    let permitted = discovery.is_some_and(|d| d.source_base == base.0 && d.supports("formPost", 1));
    if preference == PostPreference::RequireForm && !permitted {
        return Err(ExchangeError::Unsupported);
    }
    let request = base
        .request(auth, endpoint, pairs)
        .map_err(ExchangeError::Request)?;
    let mut url = request.expose_for_transport().to_owned();
    let body = if preference != PostPreference::Get && permitted {
        let index = url.find('?').ok_or(ExchangeError::Malformed)?;
        let body = url.split_off(index + 1);
        url.pop();
        Some(body)
    } else {
        None
    };
    Ok(PlannedRequest { url, body })
}
/// Transport must bound I/O while pending and arrange wakeups on cancellation/deadline.
/// The wrapper checks on every poll but cannot wake or stop an uncooperative future.
pub trait ProtocolTransport {
    fn exchange<'a>(
        &'a self,
        request: PlannedRequest,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>>;
}
pub struct TransportResponse {
    pub correlation: RequestContext,
    pub status: u16,
    pub body: Vec<u8>,
}
impl fmt::Debug for TransportResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TransportResponse([REDACTED])")
    }
}
struct Checked<'a, T> {
    inner: LocalFuture<'a, T>,
    context: OperationContext<'a>,
    clock: &'a dyn Clock,
}
impl<T> Future for Checked<'_, T> {
    type Output = Result<T, DomainError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Err(e) = self.context.check(self.clock.now()) {
            return Poll::Ready(Err(e));
        }
        match self.inner.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(v) => Poll::Ready(self.context.check(self.clock.now()).map(|()| v)),
        }
    }
}
pub(crate) async fn exchange(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    request: PlannedRequest,
) -> Result<Map<String, Value>, ExchangeError> {
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    context
        .check_request_size(request.size())
        .map_err(ExchangeError::Context)?;
    let response = Checked {
        inner: transport.exchange(request, context),
        context,
        clock,
    }
    .await
    .map_err(ExchangeError::Context)?
    .map_err(ExchangeError::Transport)?;
    if response.correlation != context.correlation {
        return Err(ExchangeError::Stale);
    }
    context
        .check_response_size(response.body.len())
        .map_err(ExchangeError::Context)?;
    if response.status != 200 {
        return Err(ExchangeError::Http(match response.status {
            401 => HttpError::Authentication,
            403 => HttpError::Forbidden,
            404 => HttpError::NotFound,
            400..=499 => HttpError::Client,
            500..=599 => HttpError::Server,
            _ => HttpError::Other,
        }));
    }
    let result = envelope(&response.body);
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
/// Ping, then query extensions only when both OpenSubsonic and a nonempty server version are advertised.
pub async fn discover(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
) -> Result<Discovery, ExchangeError> {
    let ping = exchange(
        transport,
        clock,
        context,
        plan_request(base, auth, "ping.view", &[], PostPreference::Get, None)?,
    )
    .await?;
    let protocol_version = ping
        .get("version")
        .and_then(Value::as_str)
        .ok_or(ExchangeError::Malformed)?
        .to_owned();
    let open = match ping.get("openSubsonic") {
        None => false,
        Some(v) => v.as_bool().ok_or(ExchangeError::Malformed)?,
    };
    let server_version = match ping.get("serverVersion") {
        None => None,
        Some(v) => Some(v.as_str().ok_or(ExchangeError::Malformed)?.to_owned()),
    };
    let mut result = Discovery {
        protocol_version,
        server_version,
        extensions: BTreeMap::new(),
        source_base: base.0.clone(),
    };
    if !open || result.server_version.as_deref().is_none_or(str::is_empty) {
        context.check(clock.now()).map_err(ExchangeError::Context)?;
        return Ok(result);
    }
    let payload = exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            "getOpenSubsonicExtensions.view",
            &[],
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    if let Some(entries) = payload.get("openSubsonicExtensions") {
        let entries = entries.as_array().ok_or(ExchangeError::Malformed)?;
        context
            .check_items(entries.len())
            .map_err(ExchangeError::Context)?;
        for entry in entries {
            let obj = entry.as_object().ok_or(ExchangeError::Malformed)?;
            let name = obj
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or(ExchangeError::Malformed)?;
            let versions = obj
                .get("versions")
                .and_then(Value::as_array)
                .ok_or(ExchangeError::Malformed)?;
            context
                .check_items(versions.len())
                .map_err(ExchangeError::Context)?;
            let versions = versions
                .iter()
                .map(|v| {
                    v.as_u64()
                        .filter(|n| *n > 0)
                        .ok_or(ExchangeError::Malformed)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if KNOWN.contains(&name) {
                result.extensions.insert(name.to_owned(), versions);
            }
        }
    }
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    Ok(result)
}
