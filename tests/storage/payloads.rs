use feishin_domain::{
    query_cache::{CacheInstanceId, LibraryScope, ListKind, QueryCache, QueryKey},
    *,
};
use feishin_storage::{
    imports::{QueueOccurrence, QueueSnapshot, ResumePosition},
    payloads::*,
};

fn page() -> Page<SongSummary> {
    Page {
        items: vec![SongSummary {
            identity: Scoped {
                server: ServerId::new("server"),
                id: SongId::new("song"),
            },
            name: "marker\n雪\"".into(),
            duration: Milliseconds(u64::MAX),
        }],
        start_index: u64::MAX,
        total_record_count: None,
    }
}
fn queue() -> QueueSnapshot {
    QueueSnapshot {
        occurrences: vec![
            QueueOccurrence {
                occurrence_id: "a".into(),
                song_id: "same".into(),
                server_id: "server".into(),
                title: "marker".into(),
                duration_ms: u64::MAX,
            },
            QueueOccurrence {
                occurrence_id: "b".into(),
                song_id: "same".into(),
                server_id: "server".into(),
                title: "other".into(),
                duration_ms: 2,
            },
        ],
        priority: vec!["b".into(), "a".into()],
        shuffled: vec!["b".into(), "a".into()],
        current_occurrence: Some("b".into()),
        position: Some(ResumePosition {
            occurrence_id: "b".into(),
            milliseconds: u64::MAX,
        }),
    }
}
#[test]
fn page_roundtrip_bounds_and_redaction() {
    let p = page();
    let s = ServerId::new("server");
    let v = encode_song_page(&p, &s, Limits::default()).unwrap();
    assert_eq!(
        decode_song_page(v.as_bytes(), &s, Limits::default()).unwrap(),
        p
    );
    assert_eq!(v.weight().bytes, v.as_bytes().len());
    assert_eq!(v.weight().items, 1);
    assert!(!format!("{v:?}").contains("marker"));
    let exact = Limits {
        bytes: v.as_bytes().len(),
        items: 1,
    };
    assert!(encode_song_page(&p, &s, exact).is_ok());
    assert!(decode_song_page(v.as_bytes(), &s, exact).is_ok());
    assert_eq!(
        encode_song_page(
            &p,
            &s,
            Limits {
                bytes: exact.bytes - 1,
                ..exact
            }
        )
        .unwrap_err(),
        Error::TooLarge
    );
    assert_eq!(
        decode_song_page(
            v.as_bytes(),
            &s,
            Limits {
                bytes: exact.bytes - 1,
                ..exact
            }
        )
        .unwrap_err(),
        Error::TooLarge
    );
    assert_eq!(
        encode_song_page(&p, &s, Limits { items: 0, ..exact }).unwrap_err(),
        Error::TooManyItems
    );
    assert_eq!(
        decode_song_page(v.as_bytes(), &s, Limits { items: 0, ..exact }).unwrap_err(),
        Error::TooManyItems
    );
    assert_eq!(
        encode_song_page(&p, &ServerId::new("other"), exact).unwrap_err(),
        Error::ServerMismatch
    );
    assert_eq!(
        decode_song_page(v.as_bytes(), &ServerId::new("other"), exact).unwrap_err(),
        Error::ServerMismatch
    );
    let empty = Page {
        items: vec![],
        start_index: 3,
        total_record_count: Some(0),
    };
    let l = Limits {
        bytes: 1024,
        items: 0,
    };
    let e = encode_song_page(&empty, &s, l).unwrap();
    assert_eq!(decode_song_page(e.as_bytes(), &s, l).unwrap(), empty);
    assert_eq!(
        decode_song_page(e.as_bytes(), &ServerId::new("other"), l).unwrap_err(),
        Error::ServerMismatch
    );
    assert_eq!(
        encode_song_page(&p, &s, Limits { bytes: 0, items: 1 }).unwrap_err(),
        Error::InvalidLimits
    );
}
#[test]
fn page_invalid_wire_and_string_bounds() {
    let s = ServerId::new("server");
    let l = Limits::default();
    let original = encode_song_page(&page(), &s, l).unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(original.as_bytes()).unwrap();
    wire["version"] = 2.into();
    assert_eq!(
        decode_song_page(&serde_json::to_vec(&wire).unwrap(), &s, l).unwrap_err(),
        Error::UnsupportedVersion
    );
    wire["version"] = 1.into();
    wire["items"][0]["unexpected"] = "marker".into();
    assert_eq!(
        decode_song_page(&serde_json::to_vec(&wire).unwrap(), &s, l).unwrap_err(),
        Error::InvalidPayload
    );
    wire["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("unexpected");
    for duration in [serde_json::json!(-1), serde_json::json!(1.5)] {
        wire["items"][0]["duration_ms"] = duration;
        assert_eq!(
            decode_song_page(&serde_json::to_vec(&wire).unwrap(), &s, l).unwrap_err(),
            Error::InvalidPayload
        );
    }
    wire["items"][0]["duration_ms"] = 1.into();
    for id in [" ".to_owned(), "s".repeat(129)] {
        let invalid = ServerId::new(&id);
        assert_eq!(
            encode_song_page(&page(), &invalid, l).unwrap_err(),
            Error::InvalidPayload
        );
        assert_eq!(
            decode_song_page(original.as_bytes(), &invalid, l).unwrap_err(),
            Error::InvalidPayload
        );
        wire["server_id"] = id.into();
        assert_eq!(
            decode_song_page(&serde_json::to_vec(&wire).unwrap(), &s, l).unwrap_err(),
            Error::InvalidPayload
        );
    }
    wire["server_id"] = "server".into();
    wire["items"][0]["song_id"] = "x".repeat(129).into();
    assert_eq!(
        decode_song_page(&serde_json::to_vec(&wire).unwrap(), &s, l).unwrap_err(),
        Error::InvalidPayload
    );
    let mut invalid = page();
    invalid.items[0].identity.id = SongId::new("x".repeat(129));
    assert_eq!(
        encode_song_page(&invalid, &s, l).unwrap_err(),
        Error::InvalidPayload
    );
    invalid.items[0].identity.id = SongId::new("song");
    invalid.items[0].name = "雪".repeat(342);
    assert_eq!(
        encode_song_page(&invalid, &s, l).unwrap_err(),
        Error::InvalidPayload
    );
    wire["items"][0]["song_id"] = "song".into();
    wire["items"][0]["name"] = "雪".repeat(342).into();
    assert_eq!(
        decode_song_page(&serde_json::to_vec(&wire).unwrap(), &s, l).unwrap_err(),
        Error::InvalidPayload
    );
    for limits in [
        Limits {
            bytes: MAX_BYTES + 1,
            items: 1,
        },
        Limits {
            bytes: 1,
            items: MAX_ITEMS + 1,
        },
    ] {
        assert_eq!(
            encode_song_page(&page(), &s, limits).unwrap_err(),
            Error::InvalidLimits
        );
        assert_eq!(
            decode_song_page(original.as_bytes(), &s, limits).unwrap_err(),
            Error::InvalidLimits
        );
    }
}
#[test]
fn malformed_and_queue_invariants() {
    let q = queue();
    let l = Limits::default();
    let v = encode_queue(&q, l).unwrap();
    assert_eq!(decode_queue(v.as_bytes(), l).unwrap(), q);
    assert!(!format!("{v:?}").contains("marker"));
    assert_eq!(v.weight().bytes, v.as_bytes().len());
    assert_eq!(v.weight().items, 2);
    let exact = Limits {
        bytes: v.as_bytes().len(),
        items: 2,
    };
    assert!(encode_queue(&q, exact).is_ok());
    assert!(decode_queue(v.as_bytes(), exact).is_ok());
    let short = Limits {
        bytes: exact.bytes - 1,
        ..exact
    };
    assert_eq!(encode_queue(&q, short).unwrap_err(), Error::TooLarge);
    assert_eq!(
        decode_queue(v.as_bytes(), short).unwrap_err(),
        Error::TooLarge
    );
    assert_eq!(
        decode_queue(v.as_bytes(), Limits { items: 1, ..l }).unwrap_err(),
        Error::TooManyItems
    );
    let empty = QueueSnapshot::default();
    let empty_limits = Limits {
        bytes: 1024,
        items: 0,
    };
    let empty_value = encode_queue(&empty, empty_limits).unwrap();
    assert_eq!(empty_value.weight().items, 0);
    assert_eq!(
        decode_queue(empty_value.as_bytes(), empty_limits).unwrap(),
        empty
    );
    for change in 0..4 {
        let mut bad = q.clone();
        match change {
            0 => bad.occurrences[1].occurrence_id = "a".into(),
            1 => bad.shuffled = vec!["a".into(), "a".into()],
            2 => bad.current_occurrence = Some("missing".into()),
            _ => bad.position.as_mut().unwrap().occurrence_id = "a".into(),
        }
        assert_eq!(encode_queue(&bad, l).unwrap_err(), Error::InvalidPayload);
    }
    for raw in [
        r#"{"kind":"queue_snapshot","version":2,"queue":{"occurrences":[],"priority":[],"shuffled":[],"current_occurrence":null,"position":null}}"#,
        r#"{"kind":"queue_snapshot","version":1,"extra":"marker","queue":{}}"#,
        "marker",
    ] {
        let err = decode_queue(raw.as_bytes(), l).unwrap_err();
        assert!(!format!("{err:?} {err}").contains("marker"));
    }
    assert_eq!(decode_queue(br#"{"kind":"queue_snapshot","version":2,"queue":{"occurrences":[],"priority":[],"shuffled":[],"current_occurrence":null,"position":null}}"#, l).unwrap_err(), Error::UnsupportedVersion);
    let mut invalid = q.clone();
    invalid.priority.push("missing".into());
    let wire = serde_json::json!({"kind":"queue_snapshot", "version":1, "queue": invalid});
    assert_eq!(
        decode_queue(&serde_json::to_vec(&wire).unwrap(), l).unwrap_err(),
        Error::InvalidPayload
    );
    assert_eq!(
        encode_queue(&q, Limits { items: 1, ..l }).unwrap_err(),
        Error::TooManyItems
    );
}
fn query(server: &str, start_index: u64) -> SongListQuery {
    ListQuery {
        server: ServerId::new(server),
        sort_by: SongListSort::Name,
        sort_order: SortOrder::Asc,
        start_index,
        limit: None,
        search_term: None,
        filter: SongFilter::default(),
    }
}
#[test]
fn cache_preserves_value_and_isolates_context() {
    let s = ServerId::new("server");
    let mut cache = QueryCache::new(CacheInstanceId::new(1).unwrap(), Default::default()).unwrap();
    cache.register_server(&s).unwrap();
    cache.register_server(&ServerId::new("other")).unwrap();
    let q = query("server", 0);
    let mut matching_page = page();
    matching_page.start_index = q.start_index;
    let key = QueryKey::song(&q, ListKind::List, LibraryScope(None)).unwrap();
    let ticket = cache
        .begin(key.clone(), Milliseconds(100), Milliseconds(0))
        .unwrap();
    let value = encode_song_page(&matching_page, &s, Limits::default()).unwrap();
    let weight = value.weight();
    cache
        .complete(&ticket, Ok(value), weight, Milliseconds(1))
        .unwrap();
    for other in [
        QueryKey::song(&query("server", 1), ListKind::List, LibraryScope(None)).unwrap(),
        QueryKey::song(&query("other", 0), ListKind::List, LibraryScope(None)).unwrap(),
        QueryKey::song(&q, ListKind::List, LibraryScope(Some(&[]))).unwrap(),
    ] {
        assert!(cache.get(&other, Milliseconds(2)).unwrap().is_none());
    }
    let ticket = cache
        .begin(key.clone(), Milliseconds(100), Milliseconds(3))
        .unwrap();
    cache.cancel(&ticket).unwrap();
    let v = encode_song_page(&matching_page, &s, Limits::default()).unwrap();
    let w = v.weight();
    assert!(cache.complete(&ticket, Ok(v), w, Milliseconds(4)).is_err());
    assert!(decode_song_page(b"malformed", &s, Limits::default()).is_err());
    assert!(encode_song_page(&matching_page, &s, Limits { bytes: 1, items: 1 }).is_err());
    let old = cache
        .begin(key.clone(), Milliseconds(100), Milliseconds(5))
        .unwrap();
    let newer = cache
        .begin(key.clone(), Milliseconds(100), Milliseconds(5))
        .unwrap();
    let v = encode_song_page(&matching_page, &s, Limits::default()).unwrap();
    let w = v.weight();
    assert!(cache.complete(&old, Ok(v), w, Milliseconds(6)).is_err());
    cache.cancel(&newer).unwrap();
    let (v, _) = cache.get(&key, Milliseconds(6)).unwrap().unwrap();
    assert_eq!(
        decode_song_page(v.as_bytes(), &s, Limits::default()).unwrap(),
        matching_page
    );
}
