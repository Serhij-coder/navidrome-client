use super::*;
#[test]
fn exhausted_counters_preserve_pending_and_values() {
    let s = ServerId::new("s");
    let k = QueryKey::detail(&s, LibraryItem::Song, "a", LibraryScope(None)).unwrap();
    let mut c = QueryCache::new(CacheInstanceId::new(1).unwrap(), Limits::default()).unwrap();
    c.register_server(&s).unwrap();
    let t = c
        .begin(k.clone(), Milliseconds(100), Milliseconds(0))
        .unwrap();
    c.serial = u64::MAX;
    assert!(matches!(
        c.begin(k.clone(), Milliseconds(100), Milliseconds(1)),
        Err(DomainError::Exhausted)
    ));
    assert!(!t.cancellation().is_cancelled());
    c.complete(&t, Ok(1), Weight { bytes: 1, items: 1 }, Milliseconds(1))
        .unwrap();
    c.order = u64::MAX;
    assert!(matches!(
        c.get(&k, Milliseconds(2)),
        Err(DomainError::Exhausted)
    ));
    assert_eq!(c.entries.len(), 1);
    c.servers.insert(s.clone(), u64::MAX);
    assert_eq!(c.invalidate_server(&s), Err(DomainError::Exhausted));
    assert_eq!(c.entries.len(), 1);
}
