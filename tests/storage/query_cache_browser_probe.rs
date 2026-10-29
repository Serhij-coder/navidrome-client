//! Local WASM cache fixture. No backend transport or browser storage is used.
use feishin_domain::{query_cache::*, *};
use std::rc::Rc;

// Deliberately neither Send nor Debug: cache values need neither bound.
struct Value(Rc<Vec<u8>>);
pub fn run() -> Result<bool, DomainError> {
    let a = ServerId::new("private-server-a");
    let b = ServerId::new("private-server-b");
    let folders = [FolderId::new("private-folder")];
    let mut query = SongListQuery {
        server: a.clone(),
        sort_by: SongListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: Some(20),
        search_term: Some("private-search-marker".into()),
        filter: SongFilter::default(),
    };
    let key = QueryKey::song(&query, ListKind::List, LibraryScope(Some(&folders)))?;
    let empty = QueryKey::song(&query, ListKind::List, LibraryScope(Some(&[])))?;
    let count = QueryKey::song(&query, ListKind::Count, LibraryScope(Some(&folders)))?;
    query.server = b.clone();
    let other = QueryKey::song(&query, ListKind::List, LibraryScope(Some(&folders)))?;
    let mut cache = QueryCache::new(CacheInstanceId::new(1)?, Limits::default())?;
    cache.register_server(&a)?;
    cache.register_server(&b)?;
    let old = cache.begin(key.clone(), Milliseconds(100_000), Milliseconds(0))?;
    let ticket = cache.begin(key.clone(), Milliseconds(100_000), Milliseconds(1))?;
    let superseded = old.cancellation().is_cancelled()
        && cache.complete(
            &old,
            Ok(Value(Rc::new(vec![0]))),
            Weight { bytes: 1, items: 1 },
            Milliseconds(2),
        ) == Err(DomainError::Stale);
    cache.complete(
        &ticket,
        Ok(Value(Rc::new(vec![7]))),
        Weight { bytes: 1, items: 1 },
        Milliseconds(3),
    )?;
    let fresh = cache
        .get(&key, Milliseconds(4))?
        .is_some_and(|(v, f)| v.0.as_slice() == [7] && f == Freshness::Fresh);
    let isolated = cache.get(&other, Milliseconds(5))?.is_none()
        && cache.get(&empty, Milliseconds(6))?.is_none()
        && cache.get(&count, Milliseconds(7))?.is_none();
    let stale = cache
        .get(&key, Milliseconds(10_003))?
        .is_some_and(|(_, f)| f == Freshness::Stale);
    let pending = cache.begin(key.clone(), Milliseconds(100_000), Milliseconds(10_004))?;
    cache.invalidate_family(&a, QueryFamily::Song)?;
    let invalidated = pending.cancellation().is_cancelled()
        && cache.get(&key, Milliseconds(10_005))?.is_none()
        && cache.complete(
            &pending,
            Ok(Value(Rc::new(vec![9]))),
            Weight { bytes: 1, items: 1 },
            Milliseconds(10_006),
        ) == Err(DomainError::Stale);
    let pending = cache.begin(other.clone(), Milliseconds(100_000), Milliseconds(10_007))?;
    cache.cancel(&pending)?;
    let cancelled = pending.cancellation().is_cancelled()
        && cache.complete(
            &pending,
            Ok(Value(Rc::new(vec![9]))),
            Weight { bytes: 1, items: 1 },
            Milliseconds(10_008),
        ) == Err(DomainError::Stale);
    let pending = cache.begin(other, Milliseconds(100_000), Milliseconds(10_009))?;
    let diagnostic = cache.diagnostics();
    let redacted = diagnostic.len() <= 512
        && !format!("{key:?}").contains("private-")
        && !diagnostic
            .windows(b"private-".len())
            .any(|w| w == b"private-");
    drop(cache);
    Ok(superseded
        && fresh
        && isolated
        && stale
        && invalidated
        && cancelled
        && redacted
        && pending.cancellation().is_cancelled())
}
