#![recursion_limit = "256"]
use feishin_domain::*;
use feishin_subsonic::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
struct State {
    now: Cell<u64>,
    cancelled: Cell<bool>,
}
impl Clock for State {
    fn now(&self) -> Milliseconds {
        Milliseconds(self.now.get())
    }
}
impl Cancellation for State {
    fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }
}
fn state() -> State {
    State {
        now: Cell::new(0),
        cancelled: Cell::new(false),
    }
}
fn ctx(s: &State) -> OperationContext<'_> {
    OperationContext {
        correlation: RequestContext {
            request: RequestId(4),
            generation: Generation(5),
            deadline: Milliseconds(10),
        },
        cancellation: s,
        limits: OperationLimits {
            request_bytes: 10000,
            response_bytes: 1000000,
            items: 10000,
        },
    }
}
struct Reply {
    body: Value,
    status: u16,
    header: Option<String>,
    stale: bool,
    error: Option<TransportError>,
}
fn reply(body: Value) -> Reply {
    Reply {
        body,
        status: 200,
        header: None,
        stale: false,
        error: None,
    }
}
struct Fake {
    replies: RefCell<VecDeque<Reply>>,
    urls: RefCell<Vec<String>>,
}
fn fake(replies: Vec<Reply>) -> Fake {
    Fake {
        replies: RefCell::new(replies.into()),
        urls: RefCell::new(vec![]),
    }
}
impl ProtocolTransport for Fake {
    fn exchange<'a>(
        &'a self,
        req: PlannedRequest,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
        self.urls
            .borrow_mut()
            .push(req.expose_url_for_transport().into());
        Box::pin(async move {
            let r = self
                .replies
                .borrow_mut()
                .pop_front()
                .expect("bounded fixture replies");
            if let Some(e) = r.error {
                return Err(e);
            }
            let mut body = r.body.as_object().unwrap().clone();
            body.entry("status").or_insert(json!("ok"));
            body.entry("version").or_insert(json!("1.16.1"));
            let mut correlation = c.correlation;
            if r.stale {
                correlation.request = RequestId(99);
            }
            Ok(TransportResponse {
                correlation,
                status: r.status,
                body: serde_json::to_vec(&json!({"subsonic-response":body})).unwrap(),
                total_count_header: r.header,
            })
        })
    }
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
fn run<F: Future>(f: F) -> F::Output {
    let w = Waker::from(Arc::new(Noop));
    let mut c = Context::from_waker(&w);
    match std::pin::pin!(f).as_mut().poll(&mut c) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("ready fixture"),
    }
}
#[path = "support/projection.rs"]
mod projections;
fn query() -> SongListQuery {
    ListQuery {
        server: ServerId::new("server-a"),
        sort_by: SongListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: SongFilter::default(),
    }
}
fn fixture_query(q: &Value) -> SongListQuery {
    let strings = |key: &str| -> Option<Vec<String>> {
        q.get(key).filter(|v| !v.is_null()).map(|v| {
            if let Some(s) = v.as_str() {
                vec![s.to_owned()]
            } else {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect()
            }
        })
    };
    ListQuery {
        server: ServerId::new("server-a"),
        sort_by: SongListSort::ALL
            .iter()
            .copied()
            .find(|s| Some(s.as_str()) == q["sortBy"].as_str())
            .unwrap_or(SongListSort::Name),
        sort_order: if q["sortOrder"] == "DESC" {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        },
        start_index: q["startIndex"].as_u64().unwrap_or(0),
        limit: q["limit"].as_u64(),
        search_term: q["searchTerm"].as_str().map(str::to_owned),
        filter: SongFilter {
            album_ids: strings("albumIds").map(|v| v.into_iter().map(AlbumId::new).collect()),
            album_artist_ids: strings("albumArtistIds")
                .map(|v| v.into_iter().map(ArtistId::new).collect()),
            artist_ids: strings("artistIds").map(|v| v.into_iter().map(ArtistId::new).collect()),
            genre_ids: strings("genreIds").map(|v| v.into_iter().map(GenreId::new).collect()),
            music_folder_ids: strings("musicFolderId")
                .map(|v| v.into_iter().map(FolderId::new).collect()),
            favorite: q["favorite"].as_bool(),
            min_year: q["minYear"].as_i64().map(|v| v.try_into().unwrap()),
            max_year: q["maxYear"].as_i64().map(|v| v.try_into().unwrap()),
            has_rating: q["hasRating"].as_bool(),
            image_size: q["imageSize"].as_u64().map(|v| v.try_into().unwrap()),
        },
    }
}
struct Rng {
    draws: VecDeque<u64>,
    calls: Rc<Cell<usize>>,
}
impl Random for Rng {
    fn next_u64(&mut self) -> u64 {
        self.calls.set(self.calls.get() + 1);
        self.draws.pop_front().expect("bounded fixture RNG")
    }
}
fn rng() -> Rng {
    Rng {
        draws: vec![0, u64::MAX, 1 << 63, 1 << 62].into(),
        calls: Rc::new(Cell::new(0)),
    }
}
fn list(
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    q: &SongListQuery,
    r: &mut Rng,
) -> Result<Page<Song>, ExchangeError> {
    run(get_songs(
        f,
        clock,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        q,
        r,
    ))
}
fn project_expected(v: &mut Value) {
    match v {
        Value::Array(items) => {
            for item in items {
                project_expected(item);
            }
        }
        Value::Object(obj) => {
            for item in obj.values_mut() {
                project_expected(item);
            }
        }
        _ => {}
    }
    if v["_itemType"] == "song" {
        v["duration"] = json!(v["duration"].as_f64().unwrap() as u64);
        v["playlistItemId"] = Value::Null;
        for key in ["gain", "peak"] {
            if v[key].is_object() {
                for side in ["album", "track"] {
                    if v[key].get(side).is_none() {
                        v[key][side] = Value::Null;
                    }
                }
            }
        }
    }
    if v["_itemType"] == "album" {
        v["duration"] = json!(v["duration"].as_f64().unwrap() as u64);
        if v.get("sortName").is_none_or(Value::is_null) {
            v["sortName"] = v["name"].clone();
        }
        if v.get("songCount").is_none() {
            v["songCount"] = Value::Null;
        }
    }
    if v["_itemType"] == "albumArtist" {
        for a in v["similarArtists"].as_array_mut().unwrap() {
            if a["imageId"].is_number() {
                a["imageId"] = json!(a["imageId"].to_string());
            }
        }
    }
}
#[test]
fn original_controller_all_branches_sorts_requests_and_rng_draws() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/p03/subsonic-song-lists.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let f = fake(
            case["responses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Reply {
                    status: v["status"].as_u64().unwrap_or(200).try_into().unwrap(),
                    ..reply(v["body"].clone())
                })
                .collect(),
        );
        let mut r = rng();
        r.draws = case
            .get("random_draws")
            .unwrap_or(&fixture["random_draws"])
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse::<u64>().unwrap())
            .collect();
        let result = list(&f, &s, ctx(&s), &fixture_query(&case["query"]), &mut r);
        match case["target_error"].as_str() {
            Some("Malformed") => {
                assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"])
            }
            Some("Http") => assert!(
                matches!(result, Err(ExchangeError::Http(_))),
                "{}: {result:?}",
                case["name"]
            ),
            Some("Protocol") => assert_eq!(
                result,
                Err(ExchangeError::Protocol(ProtocolError::Authentication))
            ),
            None => {
                let result = result.unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
                let actual = json!({"items":result.items.iter().map(projections::song).collect::<Vec<_>>(),"startIndex":result.start_index,"totalRecordCount":result.total_record_count});
                let mut expected = case["expected"].clone();
                project_expected(&mut expected);
                projections::equivalent(&actual, &expected);
            }
            _ => panic!("unknown target error"),
        }
        assert_eq!(
            r.calls.get() as u64,
            case["rng_calls"].as_u64().unwrap(),
            "{}",
            case["name"]
        );
        let urls = f.urls.borrow();
        let requests = case["requests"].as_array().unwrap();
        assert_eq!(urls.len(), requests.len(), "{}", case["name"]);
        for (url, request) in urls.iter().zip(requests) {
            let url = url::Url::parse(url).unwrap();
            let endpoint = request["endpoint"].as_str().unwrap();
            assert_eq!(
                url.path(),
                format!(
                    "/music/rest/{}",
                    fixture["routes"][endpoint]["path"].as_str().unwrap()
                )
            );
            let actual: std::collections::BTreeMap<_, _> = url
                .query_pairs()
                .filter(|(k, _)| !["c", "f", "v", "u", "p"].contains(&k.as_ref()))
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            let expected: std::collections::BTreeMap<_, _> = request["args"]["query"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}
fn song(id: &str) -> Value {
    json!({"id":id,"title":id,"created":"2020-01-01T00:00:00Z","contentType":"audio/flac","size":1})
}
fn branches() -> Vec<SongListQuery> {
    [
        json!({}),
        json!({"searchTerm":"x"}),
        json!({"genreIds":["g"]}),
        json!({"favorite":true}),
        json!({"albumIds":["al"]}),
        json!({"artistIds":["a"]}),
    ]
    .iter()
    .map(fixture_query)
    .collect()
}
fn empty_body() -> Value {
    json!({"searchResult3":{"song":[]},"songsByGenre":{"song":[]},"starred":{"song":[]},"album":{"song":[]},"artist":{"album":[]}})
}
#[test]
fn full_favorite_and_aggregate_fanout_work_is_bounded_before_filter_or_page() {
    let s = state();
    let mut q = fixture_query(&json!({"favorite":true,"artistIds":["none"],"limit":1}));
    let body = json!({"starred":{"song":[song("a"),song("b"),song("b")]}});
    let f = fake(vec![reply(body.clone())]);
    let mut c = ctx(&s);
    c.limits.items = 4;
    assert_eq!(
        list(&f, &s, c, &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![reply(body)]);
    c.limits.items = 5;
    assert_eq!(
        list(&f, &s, c, &q, &mut rng()).unwrap().total_record_count,
        Some(0)
    );
    q = fixture_query(&json!({"artistIds":["a","b"]}));
    let f = fake(vec![
        reply(json!({"artist":{"album":[{"id":"al"},{"id":"al"}]}})),
        reply(json!({"artist":{"album":[{"id":"al"}]}})),
    ]);
    c = ctx(&s);
    c.limits.items = 5;
    assert_eq!(
        list(&f, &s, c, &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), 2);
    q = fixture_query(&json!({"albumIds":["al","al"],"sortBy":"random","limit":0,"startIndex":10}));
    q.server = ServerId::new("other");
    let f = fake(vec![
        reply(
            json!({"album":{"song":[song("same"),song("same")],"discTitles":[{"disc":1,"title":"ignored"}]}}),
        ),
        reply(json!({"album":{"song":[song("same")]}})),
    ]);
    let mut r = rng();
    let p = list(&f, &s, ctx(&s), &q, &mut r).unwrap();
    assert_eq!(p.items.len(), 3);
    assert_eq!(p.total_record_count, Some(3));
    assert_eq!(p.start_index, 0);
    assert_eq!(r.calls.get(), 0);
    assert!(p.items.iter().all(|s| s.server == q.server
        && s.disc_subtitle.is_none()
        && s.playlist_item_id.is_none()));
}
#[test]
fn scope_selectors_pages_and_lengths_validate_before_io() {
    let s = state();
    let f = fake(vec![]);
    for mut q in branches() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            q.server = ServerId::new(server);
            assert_eq!(
                list(&f, &s, ctx(&s), &q, &mut rng()),
                Err(ExchangeError::Malformed)
            );
        }
    }
    for which in 0..8 {
        let mut q = query();
        let mut c = ctx(&s);
        c.limits.items = 2;
        match which {
            0 => q.filter.album_ids = Some(vec![AlbumId::new("a"); 3]),
            1 => q.filter.artist_ids = Some(vec![ArtistId::new("a"); 3]),
            2 => q.filter.album_artist_ids = Some(vec![ArtistId::new("a"); 3]),
            3 => q.filter.genre_ids = Some(vec![GenreId::new("g"); 3]),
            4 => q.filter.music_folder_ids = Some(vec![FolderId::new("f"); 3]),
            5 => q.search_term = Some("x".repeat(8193)),
            6 => q.start_index = 9007199254740992,
            _ => q.limit = Some(9007199254740992),
        }
        assert_eq!(
            list(&f, &s, c, &q, &mut rng()),
            Err(if which >= 6 {
                ExchangeError::Malformed
            } else {
                ExchangeError::Context(DomainError::Capacity)
            })
        );
    }
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn typed_failures_and_stale_replies_never_return_partial_lists() {
    let s = state();
    for q in branches() {
        for request in [true, false] {
            let f = fake(vec![reply(empty_body())]);
            let mut c = ctx(&s);
            if request {
                c.limits.request_bytes = 1;
            } else {
                c.limits.response_bytes = 1;
            }
            assert_eq!(
                list(&f, &s, c, &q, &mut rng()),
                Err(ExchangeError::Context(DomainError::Capacity))
            );
            assert_eq!(f.urls.borrow().len(), usize::from(!request));
        }
        let f = fake(vec![Reply {
            stale: true,
            ..reply(empty_body())
        }]);
        assert_eq!(
            list(&f, &s, ctx(&s), &q, &mut rng()),
            Err(ExchangeError::Stale)
        );
        let f = fake(vec![Reply {
            error: Some(TransportError::Network),
            ..reply(empty_body())
        }]);
        assert_eq!(
            list(&f, &s, ctx(&s), &q, &mut rng()),
            Err(ExchangeError::Transport(TransportError::Network))
        );
        let f = fake(vec![reply(
            json!({"status":"failed","error":{"code":40,"message":"private-marker"}}),
        )]);
        let e = list(&f, &s, ctx(&s), &q, &mut rng()).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    let f = fake(vec![
        reply(json!({"album":{"song":[song("same")]}})),
        Reply {
            stale: true,
            ..reply(empty_body())
        },
    ]);
    let q = fixture_query(&json!({"albumIds":["a","b"]}));
    assert_eq!(
        list(&f, &s, ctx(&s), &q, &mut rng()),
        Err(ExchangeError::Stale)
    );
}

#[test]
fn pending_and_final_context_priority_including_empty_artist_selection() {
    struct Pending;
    impl ProtocolTransport for Pending {
        fn exchange<'a>(
            &'a self,
            _: PlannedRequest,
            _: OperationContext<'a>,
        ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
            Box::pin(std::future::pending())
        }
    }
    struct Late<'a> {
        calls: Cell<u64>,
        state: &'a State,
        cancel: bool,
        after: u64,
    }
    impl Clock for Late<'_> {
        fn now(&self) -> Milliseconds {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n > self.after {
                if self.cancel {
                    self.state.cancelled.set(true);
                }
                Milliseconds(11)
            } else {
                Milliseconds(10)
            }
        }
    }
    for q in branches() {
        for cancel in [false, true] {
            let s = state();
            let base = ServerBase::parse("http://fixture.invalid").unwrap();
            let auth = Auth::legacy("dummy", "dummy").unwrap();
            let mut r = rng();
            let mut future = Box::pin(get_songs(&Pending, &s, ctx(&s), &base, &auth, &q, &mut r));
            let w = Waker::from(Arc::new(Noop));
            let mut cx = Context::from_waker(&w);
            assert!(future.as_mut().poll(&mut cx).is_pending());
            if cancel {
                s.cancelled.set(true);
            } else {
                s.now.set(11);
            }
            assert_eq!(
                future.as_mut().poll(&mut cx),
                Poll::Ready(Err(ExchangeError::Context(if cancel {
                    DomainError::Cancelled
                } else {
                    DomainError::Deadline
                })))
            );
            drop(future);
            for malformed in [false, true] {
                let s = state();
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                    after: 5,
                };
                let f = fake(vec![reply(if malformed {
                    json!({"album":3,"songsByGenre":3,"artist":3,"starred":3,"searchResult3":3})
                } else {
                    empty_body()
                })]);
                assert_eq!(
                    list(&f, &clock, ctx(&s), &q, &mut rng()),
                    Err(ExchangeError::Context(if cancel {
                        DomainError::Cancelled
                    } else {
                        DomainError::Deadline
                    }))
                );
            }
        }
    }
    let s = state();
    let clock = Late {
        calls: Cell::new(0),
        state: &s,
        cancel: false,
        after: 1,
    };
    let f = fake(vec![]);
    let mut q = query();
    q.filter.artist_ids = Some(vec![]);
    assert_eq!(
        list(&f, &clock, ctx(&s), &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Deadline))
    );
    assert!(f.urls.borrow().is_empty());
}
