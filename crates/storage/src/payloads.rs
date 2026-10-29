//! Bounded schema-v1 payloads. Cache identity and request correlation remain caller responsibilities.
use crate::{
    bounded_json,
    imports::{self, QueueSnapshot},
};
use feishin_domain::{
    Milliseconds, Page, Scoped, ServerId, SongId, SongSummary, query_cache::Weight,
};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeSeq};
use std::fmt;

pub const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_ITEMS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidLimits,
    TooLarge,
    TooManyItems,
    InvalidPayload,
    UnsupportedVersion,
    ServerMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "payload: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub bytes: usize,
    pub items: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            items: MAX_ITEMS,
        }
    }
}
impl Limits {
    fn check(self) -> Result<(), Error> {
        if self.bytes == 0 || self.bytes > MAX_BYTES || self.items > MAX_ITEMS {
            Err(Error::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

macro_rules! payload {
    ($name:ident) => {
        pub struct $name {
            bytes: Vec<u8>,
            items: usize,
        }
        impl $name {
            pub fn as_bytes(&self) -> &[u8] {
                &self.bytes
            }
            pub fn weight(&self) -> Weight {
                Weight {
                    bytes: self.bytes.len(),
                    items: self.items,
                }
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("bytes", &self.bytes.len())
                    .field("items", &self.items)
                    .finish()
            }
        }
    };
}
payload!(SongPagePayload);
payload!(QueuePayload);

fn id_valid(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 128
}
fn check_input(bytes: &[u8], limits: Limits) -> Result<(), Error> {
    limits.check()?;
    if bytes.len() > limits.bytes {
        Err(Error::TooLarge)
    } else {
        Ok(())
    }
}
fn encode<T: Serialize>(v: &T, limits: Limits) -> Result<Vec<u8>, Error> {
    bounded_json::encode(v, limits.bytes).map_err(|e| match e {
        bounded_json::EncodeError::TooLarge => Error::TooLarge,
        bounded_json::EncodeError::Invalid => Error::InvalidPayload,
    })
}
fn header(kind: &str, version: u64, expected: &str) -> Result<(), Error> {
    if version != 1 {
        return Err(Error::UnsupportedVersion);
    }
    if kind != expected {
        return Err(Error::InvalidPayload);
    }
    Ok(())
}

#[derive(Serialize)]
struct PageEnvelope<'a> {
    kind: &'static str,
    version: u64,
    server_id: &'a str,
    start_index: u64,
    total_record_count: Option<u64>,
    items: Rows<'a>,
}
struct Rows<'a>(&'a [SongSummary]);
#[derive(Serialize)]
struct RowRef<'a> {
    song_id: &'a str,
    name: &'a str,
    duration_ms: u64,
}
impl Serialize for Rows<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for row in self.0 {
            seq.serialize_element(&RowRef {
                song_id: row.identity.id.as_str(),
                name: &row.name,
                duration_ms: row.duration.0,
            })?;
        }
        seq.end()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageWire {
    kind: String,
    version: u64,
    server_id: String,
    start_index: u64,
    total_record_count: Option<u64>,
    items: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    song_id: String,
    name: String,
    duration_ms: u64,
}

pub fn encode_song_page(
    page: &Page<SongSummary>,
    server: &ServerId,
    limits: Limits,
) -> Result<SongPagePayload, Error> {
    limits.check()?;
    if !id_valid(server.as_str()) {
        return Err(Error::InvalidPayload);
    }
    if page.items.len() > limits.items {
        return Err(Error::TooManyItems);
    }
    for row in &page.items {
        if row.identity.server != *server {
            return Err(Error::ServerMismatch);
        }
        if !id_valid(row.identity.id.as_str()) || row.name.len() > 1024 {
            return Err(Error::InvalidPayload);
        }
    }
    let bytes = encode(
        &PageEnvelope {
            kind: "song_summary_page",
            version: 1,
            server_id: server.as_str(),
            start_index: page.start_index,
            total_record_count: page.total_record_count,
            items: Rows(&page.items),
        },
        limits,
    )?;
    Ok(SongPagePayload {
        bytes,
        items: page.items.len(),
    })
}
pub fn decode_song_page(
    bytes: &[u8],
    server: &ServerId,
    limits: Limits,
) -> Result<Page<SongSummary>, Error> {
    check_input(bytes, limits)?;
    if !id_valid(server.as_str()) {
        return Err(Error::InvalidPayload);
    }
    let wire: PageWire = serde_json::from_slice(bytes).map_err(|_| Error::InvalidPayload)?;
    header(&wire.kind, wire.version, "song_summary_page")?;
    if !id_valid(&wire.server_id) {
        return Err(Error::InvalidPayload);
    }
    if wire.server_id != server.as_str() {
        return Err(Error::ServerMismatch);
    }
    if wire.items.len() > limits.items {
        return Err(Error::TooManyItems);
    }
    let mut items = Vec::with_capacity(wire.items.len());
    for row in wire.items {
        if !id_valid(&row.song_id) || row.name.len() > 1024 {
            return Err(Error::InvalidPayload);
        }
        items.push(SongSummary {
            identity: Scoped {
                server: server.clone(),
                id: SongId::new(row.song_id),
            },
            name: row.name,
            duration: Milliseconds(row.duration_ms),
        });
    }
    Ok(Page {
        items,
        start_index: wire.start_index,
        total_record_count: wire.total_record_count,
    })
}
#[derive(Serialize)]
struct QueueEnvelopeRef<'a> {
    kind: &'static str,
    version: u64,
    queue: &'a QueueSnapshot,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueueEnvelope {
    kind: String,
    version: u64,
    queue: QueueSnapshot,
}
pub fn encode_queue(queue: &QueueSnapshot, limits: Limits) -> Result<QueuePayload, Error> {
    limits.check()?;
    if queue.occurrences.len() > limits.items {
        return Err(Error::TooManyItems);
    }
    imports::validate_queue(queue).map_err(|_| Error::InvalidPayload)?;
    let bytes = encode(
        &QueueEnvelopeRef {
            kind: "queue_snapshot",
            version: 1,
            queue,
        },
        limits,
    )?;
    Ok(QueuePayload {
        bytes,
        items: queue.occurrences.len(),
    })
}
pub fn decode_queue(bytes: &[u8], limits: Limits) -> Result<QueueSnapshot, Error> {
    check_input(bytes, limits)?;
    let wire: QueueEnvelope = serde_json::from_slice(bytes).map_err(|_| Error::InvalidPayload)?;
    header(&wire.kind, wire.version, "queue_snapshot")?;
    if wire.queue.occurrences.len() > limits.items {
        return Err(Error::TooManyItems);
    }
    imports::validate_queue(&wire.queue).map_err(|_| Error::InvalidPayload)?;
    Ok(wire.queue)
}
