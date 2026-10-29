//! Bounded, owner-driven query identity and cache. No network work is aborted here.
use crate::*;
use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryFamily {
    Song,
    Album,
    Artist,
    AlbumArtist,
    Genre,
    Playlist,
    User,
    Detail,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    List,
    Count,
    Infinite,
}
/// The selected library context; absent and empty remain different.
pub struct LibraryScope<'a>(pub Option<&'a [FolderId]>);
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct QueryKey {
    server: ServerId,
    family: QueryFamily,
    bytes: Vec<u8>,
}
impl fmt::Debug for QueryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryKey")
            .field("family", &self.family)
            .field("encoded_len", &self.bytes.len())
            .finish()
    }
}
struct Encoder(Vec<u8>);
impl Encoder {
    fn bytes(&mut self, s: &[u8], max: usize, nonempty: bool) -> Result<(), DomainError> {
        if s.len() > max || (nonempty && s.is_empty()) {
            return Err(DomainError::InvalidInput);
        }
        let len = u32::try_from(s.len()).map_err(|_| DomainError::Capacity)?;
        if self
            .0
            .len()
            .checked_add(4 + s.len())
            .filter(|n| *n <= 16384)
            .is_none()
        {
            return Err(DomainError::Capacity);
        }
        self.0.extend_from_slice(&len.to_le_bytes());
        self.0.extend_from_slice(s);
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), DomainError> {
        if self.0.len() >= 16384 {
            return Err(DomainError::Capacity);
        }
        self.0.push(n);
        Ok(())
    }
    fn opt<T>(
        &mut self,
        value: Option<T>,
        f: impl FnOnce(&mut Self, T) -> Result<(), DomainError>,
    ) -> Result<(), DomainError> {
        match value {
            None => self.byte(0),
            Some(v) => {
                self.byte(1)?;
                f(self, v)
            }
        }
    }
    fn ids<T>(&mut self, ids: Option<&[T]>, f: impl Fn(&T) -> &str) -> Result<(), DomainError> {
        self.opt(ids, |e, ids| {
            if ids.len() > 64 {
                return Err(DomainError::Capacity);
            }
            e.byte(ids.len() as u8)?;
            for id in ids {
                e.bytes(f(id).as_bytes(), 128, true)?;
            }
            Ok(())
        })
    }
    fn num<T: Into<i64>>(&mut self, n: Option<T>) -> Result<(), DomainError> {
        self.opt(n, |e, n| e.bytes(&n.into().to_le_bytes(), 8, false))
    }
    fn boolean(&mut self, n: Option<bool>) -> Result<(), DomainError> {
        self.opt(n, |e, n| e.byte(u8::from(n)))
    }
}
impl QueryKey {
    fn base<S, F>(
        q: &ListQuery<S, F>,
        sort: &str,
        family: QueryFamily,
        kind: ListKind,
        scope: LibraryScope<'_>,
    ) -> Result<Encoder, DomainError> {
        let mut e = Encoder(vec![1, family as u8, kind as u8]);
        e.bytes(q.server.as_str().as_bytes(), 128, true)?;
        e.bytes(sort.as_bytes(), 128, true)?;
        e.byte(q.sort_order as u8)?;
        e.bytes(&q.start_index.to_le_bytes(), 8, false)?;
        e.opt(q.limit, |e, n| e.bytes(&n.to_le_bytes(), 8, false))?;
        e.opt(q.search_term.as_deref(), |e, s| {
            e.bytes(s.as_bytes(), 1024, false)
        })?;
        e.ids(scope.0, FolderId::as_str)?;
        Ok(e)
    }
    fn finish(server: &ServerId, family: QueryFamily, e: Encoder) -> Self {
        Self {
            server: server.clone(),
            family,
            bytes: e.0,
        }
    }
    /// Identifies a detail by item, ID and library scope only; extra detail-query parameters are not represented.
    pub fn detail(
        server: &ServerId,
        item: LibraryItem,
        entity_id: &str,
        scope: LibraryScope<'_>,
    ) -> Result<Self, DomainError> {
        let mut e = Encoder(vec![1, QueryFamily::Detail as u8, item as u8]);
        e.bytes(server.as_str().as_bytes(), 128, true)?;
        e.bytes(entity_id.as_bytes(), 128, true)?;
        e.ids(scope.0, FolderId::as_str)?;
        let family = match item {
            LibraryItem::Song => QueryFamily::Song,
            LibraryItem::Album => QueryFamily::Album,
            LibraryItem::Artist => QueryFamily::Artist,
            LibraryItem::AlbumArtist => QueryFamily::AlbumArtist,
            LibraryItem::Genre => QueryFamily::Genre,
            LibraryItem::Playlist => QueryFamily::Playlist,
            _ => QueryFamily::Detail,
        };
        Ok(Self::finish(server, family, e))
    }
    pub fn song(q: &SongListQuery, k: ListKind, s: LibraryScope<'_>) -> Result<Self, DomainError> {
        let f = &q.filter;
        let mut e = Self::base(q, q.sort_by.as_str(), QueryFamily::Song, k, s)?;
        e.ids(f.album_artist_ids.as_deref(), ArtistId::as_str)?;
        e.ids(f.album_ids.as_deref(), AlbumId::as_str)?;
        e.ids(f.artist_ids.as_deref(), ArtistId::as_str)?;
        e.boolean(f.favorite)?;
        e.ids(f.genre_ids.as_deref(), GenreId::as_str)?;
        e.boolean(f.has_rating)?;
        e.num(f.image_size)?;
        e.num(f.min_year)?;
        e.num(f.max_year)?;
        e.ids(f.music_folder_ids.as_deref(), FolderId::as_str)?;
        Ok(Self::finish(&q.server, QueryFamily::Song, e))
    }
    pub fn album(
        q: &AlbumListQuery,
        k: ListKind,
        s: LibraryScope<'_>,
    ) -> Result<Self, DomainError> {
        let f = &q.filter;
        let mut e = Self::base(q, q.sort_by.as_str(), QueryFamily::Album, k, s)?;
        e.ids(f.artist_ids.as_deref(), ArtistId::as_str)?;
        e.boolean(f.compilation)?;
        e.boolean(f.favorite)?;
        e.ids(f.genre_ids.as_deref(), GenreId::as_str)?;
        e.num(f.min_year)?;
        e.num(f.max_year)?;
        e.ids(f.music_folder_ids.as_deref(), FolderId::as_str)?;
        e.boolean(f.has_rating)?;
        e.boolean(f.is_recently_played)?;
        Ok(Self::finish(&q.server, QueryFamily::Album, e))
    }
    pub fn artist(
        q: &ArtistListQuery,
        k: ListKind,
        s: LibraryScope<'_>,
    ) -> Result<Self, DomainError> {
        let f = &q.filter;
        let mut e = Self::base(q, q.sort_by.as_str(), QueryFamily::Artist, k, s)?;
        e.boolean(f.favorite)?;
        e.ids(f.music_folder_ids.as_deref(), FolderId::as_str)?;
        e.opt(f.role.as_deref(), |e, s| e.bytes(s.as_bytes(), 256, false))?;
        Ok(Self::finish(&q.server, QueryFamily::Artist, e))
    }
    pub fn album_artist(
        q: &AlbumArtistListQuery,
        k: ListKind,
        s: LibraryScope<'_>,
    ) -> Result<Self, DomainError> {
        let f = &q.filter;
        let mut e = Self::base(q, q.sort_by.as_str(), QueryFamily::AlbumArtist, k, s)?;
        e.boolean(f.favorite)?;
        e.ids(f.music_folder_ids.as_deref(), FolderId::as_str)?;
        Ok(Self::finish(&q.server, QueryFamily::AlbumArtist, e))
    }
    pub fn genre(
        q: &GenreListQuery,
        k: ListKind,
        s: LibraryScope<'_>,
    ) -> Result<Self, DomainError> {
        let mut e = Self::base(q, q.sort_by.as_str(), QueryFamily::Genre, k, s)?;
        e.ids(q.filter.music_folder_ids.as_deref(), FolderId::as_str)?;
        Ok(Self::finish(&q.server, QueryFamily::Genre, e))
    }
    pub fn playlist(
        q: &PlaylistListQuery,
        k: ListKind,
        s: LibraryScope<'_>,
    ) -> Result<Self, DomainError> {
        let mut e = Self::base(q, q.sort_by.as_str(), QueryFamily::Playlist, k, s)?;
        e.boolean(q.filter.exclude_smart_playlists)?;
        Ok(Self::finish(&q.server, QueryFamily::Playlist, e))
    }
    pub fn user(q: &UserListQuery, k: ListKind, s: LibraryScope<'_>) -> Result<Self, DomainError> {
        let e = Self::base(q, q.sort_by.as_str(), QueryFamily::User, k, s)?;
        Ok(Self::finish(&q.server, QueryFamily::User, e))
    }
}

/// Caller must assign a nonzero ID unique among replacement cache instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheInstanceId(u64);
impl CacheInstanceId {
    pub fn new(id: u64) -> Result<Self, DomainError> {
        if id == 0 {
            Err(DomainError::InvalidInput)
        } else {
            Ok(Self(id))
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub servers: usize,
    pub entries: usize,
    pub pending: usize,
    pub value_bytes: usize,
    pub value_items: usize,
    pub per_value_bytes: usize,
    pub per_value_items: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            servers: 64,
            entries: 256,
            pending: 32,
            value_bytes: 4 * 1024 * 1024,
            value_items: 10000,
            per_value_bytes: 1024 * 1024,
            per_value_items: 10000,
        }
    }
}
impl Limits {
    fn valid(self) -> bool {
        self.servers > 0
            && self.servers <= 64
            && self.entries > 0
            && self.entries <= 256
            && self.pending > 0
            && self.pending <= 32
            && self.value_bytes > 0
            && self.value_bytes <= 4 * 1024 * 1024
            && self.value_items > 0
            && self.value_items <= 10000
            && self.per_value_bytes > 0
            && self.per_value_bytes <= 1024 * 1024
            && self.per_value_items > 0
            && self.per_value_items <= 10000
    }
}
/// Trusted producer accounting for the normalized value, not heap introspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weight {
    pub bytes: usize,
    pub items: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
}
#[derive(Clone)]
pub struct CancellationToken(Arc<AtomicBool>);
impl Cancellation for CancellationToken {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
impl CancellationToken {
    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}
#[derive(Clone)]
pub struct Ticket {
    instance: CacheInstanceId,
    serial: u64,
    generation: u64,
    key: QueryKey,
    deadline: Milliseconds,
    token: CancellationToken,
}
impl Ticket {
    pub fn cancellation(&self) -> &CancellationToken {
        &self.token
    }
    pub fn context(&self) -> RequestContext {
        RequestContext {
            request: RequestId(self.serial),
            generation: Generation(self.generation),
            deadline: self.deadline,
        }
    }
    pub fn deadline(&self) -> Milliseconds {
        self.deadline
    }
}
impl fmt::Debug for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ticket")
            .field("key", &self.key)
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}
struct Entry<V> {
    value: V,
    weight: Weight,
    inserted: u64,
    used: u64,
    order: u64,
}
pub struct QueryCache<V> {
    instance: CacheInstanceId,
    limits: Limits,
    servers: HashMap<ServerId, u64>,
    entries: HashMap<QueryKey, Entry<V>>,
    pending: HashMap<QueryKey, Ticket>,
    serial: u64,
    order: u64,
    now: u64,
    bytes: usize,
    items: usize,
}
impl<V> Drop for QueryCache<V> {
    fn drop(&mut self) {
        for ticket in self.pending.values() {
            ticket.token.cancel();
        }
    }
}
impl<V> QueryCache<V> {
    pub fn new(instance: CacheInstanceId, limits: Limits) -> Result<Self, DomainError> {
        if !limits.valid() {
            return Err(DomainError::InvalidInput);
        }
        Ok(Self {
            instance,
            limits,
            servers: HashMap::new(),
            entries: HashMap::new(),
            pending: HashMap::new(),
            serial: 0,
            order: 0,
            now: 0,
            bytes: 0,
            items: 0,
        })
    }
    fn clock(&mut self, now: Milliseconds) -> Result<(), DomainError> {
        if now.0 < self.now {
            return Err(DomainError::InvalidInput);
        }
        self.now = now.0;
        Ok(())
    }
    pub fn register_server(&mut self, server: &ServerId) -> Result<(), DomainError> {
        if server.as_str().is_empty() || server.as_str().len() > 128 {
            return Err(DomainError::InvalidInput);
        }
        if !self.servers.contains_key(server) && self.servers.len() >= self.limits.servers {
            return Err(DomainError::Capacity);
        }
        self.servers.entry(server.clone()).or_insert(0);
        Ok(())
    }
    fn remove_entry(&mut self, key: &QueryKey) {
        if let Some(e) = self.entries.remove(key) {
            self.bytes -= e.weight.bytes;
            self.items -= e.weight.items;
        }
    }
    fn cancel_where(&mut self, predicate: impl Fn(&QueryKey) -> bool) {
        self.pending.retain(|k, t| {
            if predicate(k) {
                t.token.cancel();
                false
            } else {
                true
            }
        });
    }
    fn clear_where(&mut self, predicate: impl Fn(&QueryKey) -> bool) {
        self.cancel_where(&predicate);
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|k| predicate(k))
            .cloned()
            .collect();
        for k in keys {
            self.remove_entry(&k);
        }
    }
    pub fn maintain(&mut self, now: Milliseconds) -> Result<(), DomainError> {
        self.clock(now)?;
        self.pending.retain(|_, t| {
            if now.0 >= t.deadline.0 {
                t.token.cancel();
                false
            } else {
                true
            }
        });
        let keys: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| now.0 - e.used >= 20_000)
            .map(|(k, _)| k.clone())
            .collect();
        for k in keys {
            self.remove_entry(&k);
        }
        Ok(())
    }
    pub fn begin(
        &mut self,
        key: QueryKey,
        deadline: Milliseconds,
        now: Milliseconds,
    ) -> Result<Ticket, DomainError> {
        if now.0 < self.now {
            return Err(DomainError::InvalidInput);
        }
        if deadline.0 <= now.0 {
            return Err(DomainError::Deadline);
        }
        let generation = *self
            .servers
            .get(&key.server)
            .ok_or(DomainError::InvalidInput)?;
        let serial = self.serial.checked_add(1).ok_or(DomainError::Exhausted)?;
        let live = self
            .pending
            .values()
            .filter(|t| t.deadline.0 > now.0)
            .count();
        if self.pending.get(&key).is_none_or(|t| t.deadline.0 <= now.0)
            && live >= self.limits.pending
        {
            return Err(DomainError::Capacity);
        }
        self.maintain(now)?;
        let t = Ticket {
            instance: self.instance,
            serial,
            generation,
            key: key.clone(),
            deadline,
            token: CancellationToken(Arc::new(AtomicBool::new(false))),
        };
        if let Some(old) = self.pending.insert(key, t.clone()) {
            old.token.cancel();
        }
        self.serial = serial;
        Ok(t)
    }
    pub fn complete(
        &mut self,
        ticket: &Ticket,
        result: Result<V, DomainError>,
        weight: Weight,
        now: Milliseconds,
    ) -> Result<(), DomainError> {
        if now.0 < self.now {
            return Err(DomainError::InvalidInput);
        }
        if ticket.instance != self.instance
            || !self
                .pending
                .get(&ticket.key)
                .is_some_and(|t| t.serial == ticket.serial && t.generation == ticket.generation)
        {
            return Err(DomainError::Stale);
        }
        self.clock(now)?;
        self.pending.remove(&ticket.key);
        if ticket.token.is_cancelled() {
            return Err(DomainError::Cancelled);
        }
        if now.0 >= ticket.deadline.0 {
            ticket.token.cancel();
            return Err(DomainError::Deadline);
        }
        if self.servers.get(&ticket.key.server) != Some(&ticket.generation) {
            return Err(DomainError::Stale);
        }
        let value = result?;
        if weight.bytes > self.limits.per_value_bytes
            || weight.bytes > self.limits.value_bytes
            || weight.items > self.limits.per_value_items
            || weight.items > self.limits.value_items
        {
            return Err(DomainError::Capacity);
        }
        let order = self.order.checked_add(1).ok_or(DomainError::Exhausted)?;
        self.remove_entry(&ticket.key);
        while self.entries.len() >= self.limits.entries
            || self
                .bytes
                .checked_add(weight.bytes)
                .is_none_or(|v| v > self.limits.value_bytes)
            || self
                .items
                .checked_add(weight.items)
                .is_none_or(|v| v > self.limits.value_items)
        {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.order)
                .map(|(k, _)| k.clone())
            else {
                return Err(DomainError::Capacity);
            };
            self.remove_entry(&key);
        }
        self.bytes += weight.bytes;
        self.items += weight.items;
        self.order = order;
        self.entries.insert(
            ticket.key.clone(),
            Entry {
                value,
                weight,
                inserted: now.0,
                used: now.0,
                order,
            },
        );
        Ok(())
    }
    pub fn get(
        &mut self,
        key: &QueryKey,
        now: Milliseconds,
    ) -> Result<Option<(&V, Freshness)>, DomainError> {
        if now.0 < self.now {
            return Err(DomainError::InvalidInput);
        }
        let order = if self.entries.contains_key(key) {
            Some(self.order.checked_add(1).ok_or(DomainError::Exhausted)?)
        } else {
            None
        };
        self.maintain(now)?;
        if let (Some(order), Some(e)) = (order, self.entries.get_mut(key)) {
            self.order = order;
            e.order = order;
            e.used = now.0;
            let freshness = if now.0 - e.inserted < 10_000 {
                Freshness::Fresh
            } else {
                Freshness::Stale
            };
            Ok(Some((&e.value, freshness)))
        } else {
            Ok(None)
        }
    }
    pub fn invalidate_server(&mut self, server: &ServerId) -> Result<(), DomainError> {
        let generation = self
            .servers
            .get(server)
            .ok_or(DomainError::InvalidInput)?
            .checked_add(1)
            .ok_or(DomainError::Exhausted)?;
        self.clear_where(|k| &k.server == server);
        self.servers.insert(server.clone(), generation);
        Ok(())
    }
    pub fn invalidate_family(
        &mut self,
        server: &ServerId,
        family: QueryFamily,
    ) -> Result<(), DomainError> {
        if !self.servers.contains_key(server) {
            return Err(DomainError::InvalidInput);
        }
        self.clear_where(|k| &k.server == server && k.family == family);
        Ok(())
    }
    /// Cancels only the exact currently pending ticket, leaving cached data intact.
    pub fn cancel(&mut self, ticket: &Ticket) -> Result<(), DomainError> {
        if ticket.instance != self.instance
            || !self
                .pending
                .get(&ticket.key)
                .is_some_and(|t| t.serial == ticket.serial && t.generation == ticket.generation)
        {
            return Err(DomainError::Stale);
        }
        if let Some(pending) = self.pending.remove(&ticket.key) {
            pending.token.cancel();
        }
        Ok(())
    }
    pub fn cancel_server(&mut self, server: &ServerId) -> Result<(), DomainError> {
        if !self.servers.contains_key(server) {
            return Err(DomainError::InvalidInput);
        }
        self.cancel_where(|k| &k.server == server);
        Ok(())
    }
    pub fn remove_server(&mut self, server: &ServerId) -> Result<(), DomainError> {
        if !self.servers.contains_key(server) {
            return Err(DomainError::InvalidInput);
        }
        self.clear_where(|k| &k.server == server);
        self.servers.remove(server);
        Ok(())
    }
    /// Schema 1 fixed-size counters only; keys and values never appear. The last
    /// counter accounts encoded keys of retained entries only, excluding pending
    /// copies and other heap overhead.
    pub fn diagnostics(&self) -> Vec<u8> {
        let mut out = vec![1];
        for n in [
            self.servers.len(),
            self.entries.len(),
            self.pending.len(),
            self.bytes,
            self.items,
            self.entries.keys().map(|k| k.bytes.len()).sum(),
        ] {
            out.extend_from_slice(&(n as u64).to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
#[path = "../../../tests/domain/query_cache_internals.rs"]
mod tests;
