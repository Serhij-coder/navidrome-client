use feishin_domain::{query_cache::*, *};
use std::rc::Rc;
fn song(server: &str) -> SongListQuery {
    ListQuery {
        server: ServerId::new(server),
        sort_by: SongListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: SongFilter::default(),
    }
}
fn key(q: &SongListQuery) -> QueryKey {
    QueryKey::song(q, ListKind::List, LibraryScope(None)).unwrap()
}
fn cache() -> QueryCache<Rc<u32>> {
    QueryCache::new(CacheInstanceId::new(1).unwrap(), Limits::default()).unwrap()
}
fn put(c: &mut QueryCache<Rc<u32>>, k: QueryKey, value: u32, now: u64) {
    let t = c
        .begin(k, Milliseconds(now + 100), Milliseconds(now))
        .unwrap();
    c.complete(
        &t,
        Ok(Rc::new(value)),
        Weight { bytes: 1, items: 1 },
        Milliseconds(now),
    )
    .unwrap();
}
#[test]
fn query_identity_and_bounds() {
    let q = song("secret-server");
    let original = key(&q);
    assert_ne!(
        original,
        QueryKey::song(&q, ListKind::Count, LibraryScope(None)).unwrap()
    );
    assert_ne!(
        original,
        QueryKey::song(&q, ListKind::Infinite, LibraryScope(None)).unwrap()
    );
    assert_ne!(
        original,
        QueryKey::song(&q, ListKind::List, LibraryScope(Some(&[]))).unwrap()
    );
    let mut variants = Vec::new();
    let mut v = q.clone();
    v.server = ServerId::new("other");
    variants.push(v);
    let mut v = q.clone();
    v.sort_by = SongListSort::Album;
    variants.push(v);
    let mut v = q.clone();
    v.sort_order = SortOrder::Desc;
    variants.push(v);
    let mut v = q.clone();
    v.start_index = 1;
    variants.push(v);
    let mut v = q.clone();
    v.limit = Some(0);
    variants.push(v);
    let mut v = q.clone();
    v.search_term = Some(String::new());
    variants.push(v);
    let mut v = q.clone();
    v.filter.album_artist_ids = Some(vec![ArtistId::new("x")]);
    variants.push(v);
    let mut v = q.clone();
    v.filter.album_ids = Some(vec![AlbumId::new("x")]);
    variants.push(v);
    let mut v = q.clone();
    v.filter.artist_ids = Some(vec![ArtistId::new("x")]);
    variants.push(v);
    let mut v = q.clone();
    v.filter.favorite = Some(false);
    variants.push(v);
    let mut v = q.clone();
    v.filter.genre_ids = Some(vec![GenreId::new("x")]);
    variants.push(v);
    let mut v = q.clone();
    v.filter.has_rating = Some(false);
    variants.push(v);
    let mut v = q.clone();
    v.filter.image_size = Some(0);
    variants.push(v);
    let mut v = q.clone();
    v.filter.min_year = Some(0);
    variants.push(v);
    let mut v = q.clone();
    v.filter.max_year = Some(0);
    variants.push(v);
    let mut v = q.clone();
    v.filter.music_folder_ids = Some(vec![]);
    variants.push(v);
    for v in variants {
        assert_ne!(original, key(&v));
    }
    let mut v = q.clone();
    v.search_term = Some("private-marker".repeat(100));
    assert!(QueryKey::song(&v, ListKind::List, LibraryScope(None)).is_err());
    let mut v = q.clone();
    v.filter.album_ids = Some(vec![AlbumId::new("x"); 65]);
    assert!(QueryKey::song(&v, ListKind::List, LibraryScope(None)).is_err());
    assert!(!format!("{original:?}").contains("secret-server"));
    assert_ne!(
        QueryKey::detail(
            &ServerId::new("a"),
            LibraryItem::Song,
            "x",
            LibraryScope(None)
        )
        .unwrap(),
        QueryKey::detail(
            &ServerId::new("b"),
            LibraryItem::Song,
            "x",
            LibraryScope(None)
        )
        .unwrap()
    );
}
#[test]
fn freshness_idle_and_lru() {
    let limits = Limits {
        entries: 2,
        ..Limits::default()
    };
    let mut c = QueryCache::new(CacheInstanceId::new(9).unwrap(), limits).unwrap();
    let s = ServerId::new("s");
    c.register_server(&s).unwrap();
    let a = key(&song("s"));
    let mut q = song("s");
    q.start_index = 1;
    let b = key(&q);
    q.start_index = 2;
    let d = key(&q);
    put(&mut c, a.clone(), 1, 0);
    put(&mut c, b.clone(), 2, 1);
    assert_eq!(
        c.get(&a, Milliseconds(10_000)).unwrap().unwrap().1,
        Freshness::Stale
    );
    put(&mut c, d.clone(), 3, 10_001);
    assert!(c.get(&b, Milliseconds(10_001)).unwrap().is_none());
    assert!(c.get(&a, Milliseconds(30_000)).unwrap().is_none());
    assert!(c.get(&d, Milliseconds(30_000)).unwrap().is_some());
}
#[test]
fn correlation_and_invalidation() {
    let mut c = cache();
    let s = ServerId::new("s");
    c.register_server(&s).unwrap();
    let k = key(&song("s"));
    let t = c
        .begin(k.clone(), Milliseconds(20), Milliseconds(0))
        .unwrap();
    let t2 = c
        .begin(k.clone(), Milliseconds(20), Milliseconds(1))
        .unwrap();
    assert!(t.cancellation().is_cancelled());
    assert_eq!(
        c.complete(
            &t,
            Ok(Rc::new(1)),
            Weight { bytes: 1, items: 1 },
            Milliseconds(2)
        ),
        Err(DomainError::Stale)
    );
    c.invalidate_family(&s, QueryFamily::Song).unwrap();
    assert!(t2.cancellation().is_cancelled());
    assert_eq!(
        c.complete(
            &t2,
            Ok(Rc::new(2)),
            Weight { bytes: 1, items: 1 },
            Milliseconds(2)
        ),
        Err(DomainError::Stale)
    );
    let t = c
        .begin(k.clone(), Milliseconds(20), Milliseconds(2))
        .unwrap();
    c.remove_server(&s).unwrap();
    c.register_server(&s).unwrap();
    let next = c
        .begin(k.clone(), Milliseconds(20), Milliseconds(3))
        .unwrap();
    assert_eq!(
        c.complete(
            &t,
            Ok(Rc::new(1)),
            Weight { bytes: 1, items: 1 },
            Milliseconds(3)
        ),
        Err(DomainError::Stale)
    );
    assert_eq!(
        c.complete(
            &next,
            Ok(Rc::new(1)),
            Weight { bytes: 1, items: 1 },
            Milliseconds(20)
        ),
        Err(DomainError::Deadline)
    );
    assert!(c.get(&k, Milliseconds(19)).is_err());
    assert!(c.get(&k, Milliseconds(20)).unwrap().is_none());
}
#[test]
fn failed_completion_preserves_value_and_diagnostics_redacted() {
    let mut c = cache();
    c.register_server(&ServerId::new("s")).unwrap();
    let k = key(&song("s"));
    put(&mut c, k.clone(), 7, 0);
    let t = c
        .begin(k.clone(), Milliseconds(30), Milliseconds(1))
        .unwrap();
    assert_eq!(
        c.complete(
            &t,
            Ok(Rc::new(8)),
            Weight {
                bytes: usize::MAX,
                items: 1
            },
            Milliseconds(2)
        ),
        Err(DomainError::Capacity)
    );
    assert_eq!(**c.get(&k, Milliseconds(2)).unwrap().unwrap().0, 7);
    let t = c
        .begin(k.clone(), Milliseconds(30), Milliseconds(3))
        .unwrap();
    assert_eq!(
        c.complete(
            &t,
            Err(DomainError::Network),
            Weight { bytes: 0, items: 0 },
            Milliseconds(4)
        ),
        Err(DomainError::Network)
    );
    assert_eq!(**c.get(&k, Milliseconds(4)).unwrap().unwrap().0, 7);
    assert_eq!(c.diagnostics().len(), 49);
    assert!(!c.diagnostics().windows(6).any(|w| w == b"secret"));
}
#[test]
fn every_list_family_and_filter_is_distinct() {
    let server = ServerId::new("s");
    let mut a = AlbumListQuery {
        server: server.clone(),
        sort_by: AlbumListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: AlbumFilter::default(),
    };
    let original = QueryKey::album(&a, ListKind::Count, LibraryScope(None)).unwrap();
    macro_rules! album_variant {
        ($field:ident,$value:expr) => {{
            let mut v = a.clone();
            v.filter.$field = Some($value);
            assert_ne!(
                original,
                QueryKey::album(&v, ListKind::Count, LibraryScope(None)).unwrap()
            );
        }};
    }
    album_variant!(artist_ids, vec![]);
    album_variant!(compilation, false);
    album_variant!(favorite, false);
    album_variant!(genre_ids, vec![]);
    album_variant!(min_year, 0);
    album_variant!(max_year, 0);
    album_variant!(music_folder_ids, vec![]);
    album_variant!(has_rating, false);
    album_variant!(is_recently_played, false);
    a.start_index = 1;
    assert_ne!(
        original,
        QueryKey::album(&a, ListKind::Count, LibraryScope(None)).unwrap()
    );
    let mut artist = ArtistListQuery {
        server: server.clone(),
        sort_by: ArtistListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: ArtistFilter::default(),
    };
    let base = QueryKey::artist(&artist, ListKind::List, LibraryScope(None)).unwrap();
    artist.filter.role = Some(String::new());
    assert_ne!(
        base,
        QueryKey::artist(&artist, ListKind::List, LibraryScope(None)).unwrap()
    );
    artist.filter.role = Some("r".repeat(257));
    assert!(QueryKey::artist(&artist, ListKind::List, LibraryScope(None)).is_err());
    let aa = AlbumArtistListQuery {
        server: server.clone(),
        sort_by: AlbumArtistListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: AlbumArtistFilter::default(),
    };
    let mut aa2 = aa.clone();
    aa2.filter.favorite = Some(false);
    assert_ne!(
        QueryKey::album_artist(&aa, ListKind::List, LibraryScope(None)).unwrap(),
        QueryKey::album_artist(&aa2, ListKind::List, LibraryScope(None)).unwrap()
    );
    let g = GenreListQuery {
        server: server.clone(),
        sort_by: GenreListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: GenreFilter::default(),
    };
    let mut g2 = g.clone();
    g2.filter.music_folder_ids = Some(vec![]);
    assert_ne!(
        QueryKey::genre(&g, ListKind::List, LibraryScope(None)).unwrap(),
        QueryKey::genre(&g2, ListKind::List, LibraryScope(None)).unwrap()
    );
    let p = PlaylistListQuery {
        server: server.clone(),
        sort_by: PlaylistListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: PlaylistFilter::default(),
    };
    let mut p2 = p.clone();
    p2.filter.exclude_smart_playlists = Some(false);
    assert_ne!(
        QueryKey::playlist(&p, ListKind::List, LibraryScope(None)).unwrap(),
        QueryKey::playlist(&p2, ListKind::List, LibraryScope(None)).unwrap()
    );
    let u = UserListQuery {
        server,
        sort_by: UserListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: (),
    };
    assert_ne!(
        QueryKey::user(&u, ListKind::List, LibraryScope(None)).unwrap(),
        QueryKey::user(&u, ListKind::Infinite, LibraryScope(None)).unwrap()
    );
}
#[test]
fn cross_instance_cancel_and_capacity() {
    let limits = Limits {
        value_bytes: 2,
        value_items: 2,
        pending: 1,
        ..Limits::default()
    };
    let mut a: QueryCache<i32> = QueryCache::new(CacheInstanceId::new(1).unwrap(), limits).unwrap();
    let mut b = QueryCache::new(CacheInstanceId::new(2).unwrap(), limits).unwrap();
    let s = ServerId::new("s");
    a.register_server(&s).unwrap();
    b.register_server(&s).unwrap();
    let k = key(&song("s"));
    let t = a
        .begin(k.clone(), Milliseconds(10), Milliseconds(0))
        .unwrap();
    let other = b
        .begin(k.clone(), Milliseconds(10), Milliseconds(0))
        .unwrap();
    assert_eq!(
        b.complete(&t, Ok(1), Weight { bytes: 1, items: 1 }, Milliseconds(1)),
        Err(DomainError::Stale)
    );
    assert!(!other.cancellation().is_cancelled());
    let mut q = song("s");
    q.start_index = 1;
    assert!(matches!(
        a.begin(key(&q), Milliseconds(10), Milliseconds(0)),
        Err(DomainError::Capacity)
    ));
    assert!(!t.cancellation().is_cancelled());
    a.cancel_server(&s).unwrap();
    assert!(t.cancellation().is_cancelled());
}
#[test]
fn generated_server_invalidations_do_not_resurrect() {
    let mut c = QueryCache::new(
        CacheInstanceId::new(42).unwrap(),
        Limits {
            entries: 3,
            value_bytes: 3,
            value_items: 3,
            ..Limits::default()
        },
    )
    .unwrap();
    let servers = [ServerId::new("a"), ServerId::new("b")];
    for s in &servers {
        c.register_server(s).unwrap();
    }
    let mut seed = 7_u64;
    for now in 0..300_u64 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let s = &servers[(seed as usize) & 1];
        let mut q = song(s.as_str());
        q.start_index = (seed >> 8) % 5;
        let k = key(&q);
        let t = c
            .begin(k.clone(), Milliseconds(now + 1), Milliseconds(now))
            .unwrap();
        if seed & 4 != 0 {
            c.invalidate_server(s).unwrap();
            assert_eq!(
                c.complete(
                    &t,
                    Ok(now),
                    Weight { bytes: 1, items: 1 },
                    Milliseconds(now)
                ),
                Err(DomainError::Stale)
            );
            assert!(c.get(&k, Milliseconds(now)).unwrap().is_none());
        } else {
            c.complete(
                &t,
                Ok(now),
                Weight { bytes: 1, items: 1 },
                Milliseconds(now),
            )
            .unwrap();
            assert_eq!(*c.get(&k, Milliseconds(now)).unwrap().unwrap().0, now);
        }
        let d = c.diagnostics();
        let read = |at| u64::from_le_bytes(d[at..at + 8].try_into().unwrap());
        assert!(read(9) <= 3 && read(25) <= 3 && read(33) <= 3);
    }
}
#[test]
fn family_invalidation_covers_details_without_affecting_other_families() {
    let s = ServerId::new("s");
    let mut c = cache();
    c.register_server(&s).unwrap();
    let detail = QueryKey::detail(&s, LibraryItem::Song, "same", LibraryScope(None)).unwrap();
    let album = QueryKey::detail(&s, LibraryItem::Album, "same", LibraryScope(None)).unwrap();
    put(&mut c, detail.clone(), 1, 0);
    put(&mut c, album.clone(), 2, 0);
    c.invalidate_family(&s, QueryFamily::Song).unwrap();
    assert!(c.get(&detail, Milliseconds(0)).unwrap().is_none());
    assert!(c.get(&album, Milliseconds(0)).unwrap().is_some());
}
#[test]
fn detail_scope_and_exact_cancellation() {
    let s = ServerId::new("s");
    let empty = QueryKey::detail(&s, LibraryItem::Song, "id", LibraryScope(Some(&[]))).unwrap();
    let absent = QueryKey::detail(&s, LibraryItem::Song, "id", LibraryScope(None)).unwrap();
    let folders = [FolderId::new("folder")];
    let selected =
        QueryKey::detail(&s, LibraryItem::Song, "id", LibraryScope(Some(&folders))).unwrap();
    assert_ne!(empty, absent);
    assert_ne!(selected, absent);
    let mut cache: QueryCache<u32> =
        QueryCache::new(CacheInstanceId::new(1).unwrap(), Limits::default()).unwrap();
    let mut foreign: QueryCache<u32> =
        QueryCache::new(CacheInstanceId::new(2).unwrap(), Limits::default()).unwrap();
    cache.register_server(&s).unwrap();
    foreign.register_server(&s).unwrap();
    let first = cache
        .begin(absent.clone(), Milliseconds(10), Milliseconds(0))
        .unwrap();
    let second = cache
        .begin(selected.clone(), Milliseconds(11), Milliseconds(0))
        .unwrap();
    let other = foreign
        .begin(absent.clone(), Milliseconds(10), Milliseconds(0))
        .unwrap();
    assert_eq!(first.context().deadline, Milliseconds(10));
    assert_eq!(first.context().request, RequestId(1));
    assert_eq!(first.context().generation, Generation(0));
    assert_eq!(cache.cancel(&other), Err(DomainError::Stale));
    assert!(!first.cancellation().is_cancelled());
    cache.cancel(&first).unwrap();
    assert!(first.cancellation().is_cancelled());
    assert!(!second.cancellation().is_cancelled());
    assert_eq!(
        cache.complete(
            &first,
            Ok(1),
            Weight { bytes: 1, items: 1 },
            Milliseconds(1)
        ),
        Err(DomainError::Stale)
    );
    cache
        .complete(
            &second,
            Ok(2),
            Weight { bytes: 1, items: 1 },
            Milliseconds(1),
        )
        .unwrap();
    assert_eq!(
        *cache.get(&selected, Milliseconds(1)).unwrap().unwrap().0,
        2
    );
}
#[test]
fn dropping_owner_cancels_pending_and_replacement_rejects_ticket() {
    let server = ServerId::new("s");
    let key = key(&song("s"));
    let old = {
        let mut owner: QueryCache<u32> =
            QueryCache::new(CacheInstanceId::new(10).unwrap(), Limits::default()).unwrap();
        owner.register_server(&server).unwrap();
        owner
            .begin(key.clone(), Milliseconds(10), Milliseconds(0))
            .unwrap()
    };
    assert!(old.cancellation().is_cancelled());
    let mut replacement: QueryCache<u32> =
        QueryCache::new(CacheInstanceId::new(11).unwrap(), Limits::default()).unwrap();
    replacement.register_server(&server).unwrap();
    let current = replacement
        .begin(key, Milliseconds(10), Milliseconds(0))
        .unwrap();
    assert_eq!(
        replacement.complete(&old, Ok(1), Weight { bytes: 1, items: 1 }, Milliseconds(1)),
        Err(DomainError::Stale)
    );
    assert!(!current.cancellation().is_cancelled());
    replacement
        .complete(
            &current,
            Ok(2),
            Weight { bytes: 1, items: 1 },
            Milliseconds(1),
        )
        .unwrap();
}
