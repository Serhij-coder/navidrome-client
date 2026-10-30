use feishin_domain::*;
use feishin_subsonic::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
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
fn query() -> AlbumListQuery {
    ListQuery {
        server: ServerId::new("server-a"),
        sort_by: AlbumListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: AlbumFilter::default(),
    }
}
fn count(
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    q: &AlbumListQuery,
) -> Result<u64, ExchangeError> {
    run(get_album_count(
        f,
        clock,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        q,
        2026,
    ))
}
#[test]
fn repeated_pages_header_shortcut_and_artist_reset() {
    let s = state();
    let f = fake(vec![
        reply(json!({"albumList2":{"album":vec![Value::Null;500]}})),
        reply(json!({"albumList2":{"album":[null,null]}})),
    ]);
    assert_eq!(count(&f, &s, ctx(&s), &query()), Ok(502));
    assert!(f.urls.borrow()[1].contains("offset=500"));
    let f = fake(vec![Reply {
        header: Some("1200".into()),
        ..reply(json!({}))
    }]);
    assert_eq!(count(&f, &s, ctx(&s), &query()), Ok(1200));
    assert_eq!(f.urls.borrow().len(), 1);
    let mut q = query();
    q.filter.artist_ids = Some(vec![
        ArtistId::new("a"),
        ArtistId::new("b"),
        ArtistId::new("c"),
    ]);
    let f = fake(vec![
        reply(json!({"artist":{"album":[null,null]}})),
        Reply {
            status: 500,
            ..reply(json!({}))
        },
        reply(json!({"artist":{"album":[null]}})),
    ]);
    assert_eq!(count(&f, &s, ctx(&s), &q), Ok(1));
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-album-count.json")).unwrap()
}
fn strings<I>(q: &Value, key: &str, build: impl Fn(&str) -> I) -> Option<Vec<I>> {
    q.get(key).filter(|v| !v.is_null()).map(|v| {
        if let Some(s) = v.as_str() {
            vec![build(s)]
        } else {
            v.as_array()
                .unwrap()
                .iter()
                .map(|v| build(v.as_str().unwrap()))
                .collect()
        }
    })
}
fn fixture_query(q: &Value) -> AlbumListQuery {
    ListQuery {
        server: ServerId::new("server-a"),
        sort_by: AlbumListSort::ALL
            .iter()
            .copied()
            .find(|s| Some(s.as_str()) == q["sortBy"].as_str())
            .unwrap_or(AlbumListSort::Name),
        sort_order: if q["sortOrder"] == "DESC" {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        },
        start_index: q["startIndex"].as_u64().unwrap_or(0),
        limit: q["limit"].as_u64(),
        search_term: q["searchTerm"].as_str().map(str::to_owned),
        filter: AlbumFilter {
            artist_ids: strings(q, "artistIds", |s| ArtistId::new(s)),
            genre_ids: strings(q, "genreIds", |s| GenreId::new(s)),
            music_folder_ids: strings(q, "musicFolderId", |s| FolderId::new(s)),
            min_year: q["minYear"].as_i64().map(|n| n.try_into().unwrap()),
            max_year: q["maxYear"].as_i64().map(|n| n.try_into().unwrap()),
            favorite: q["favorite"].as_bool(),
            compilation: q["compilation"].as_bool(),
            has_rating: q["hasRating"].as_bool(),
            is_recently_played: q["isRecentlyPlayed"].as_bool(),
        },
    }
}
#[test]
fn source_controller_all_branches_requests_and_characterized_differences() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let q = fixture_query(&case["query"]);
        let replies = case["responses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Reply {
                status: v["status"].as_u64().unwrap_or(200).try_into().unwrap(),
                header: v["header"].as_str().map(str::to_owned),
                ..reply(v["body"].clone())
            })
            .collect();
        let f = fake(replies);
        let result = run(get_album_count(
            &f,
            &s,
            ctx(&s),
            &ServerBase::parse("http://fixture.invalid/music").unwrap(),
            &Auth::legacy("dummy", "dummy").unwrap(),
            &q,
            case["current_year"].as_i64().unwrap().try_into().unwrap(),
        ));
        match case["target_error"].as_str() {
            Some("Malformed") => {
                assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"])
            }
            Some("Http") => assert_eq!(
                result,
                Err(ExchangeError::Http(HttpError::Server)),
                "{}",
                case["name"]
            ),
            Some("Protocol") => assert_eq!(
                result,
                Err(ExchangeError::Protocol(ProtocolError::Authentication)),
                "{}",
                case["name"]
            ),
            _ => assert_eq!(
                result,
                Ok(case["expected"].as_u64().unwrap()),
                "{}",
                case["name"]
            ),
        }
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
#[test]
fn bounded_changing_library_metadata_and_selected_inputs() {
    let s = state();
    let full = || reply(json!({"albumList2":{"album":vec![Value::Null;500]}}));
    let f = fake(vec![full(), full(), full()]);
    let mut c = ctx(&s);
    c.limits.items = 1001;
    assert_eq!(
        count(&f, &s, c, &query()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), 3);
    let f = fake(vec![full(), reply(json!({"albumList2":{"album":[]}}))]);
    c.limits.items = 501;
    assert_eq!(count(&f, &s, c, &query()), Ok(500));
    let f = fake(vec![Reply {
        header: Some("0".into()),
        ..reply(json!({}))
    }]);
    c.limits.items = 1;
    assert_eq!(count(&f, &s, c, &query()), Ok(0));
    let f = fake(vec![Reply {
        header: Some("1".repeat(129)),
        ..reply(json!({}))
    }]);
    assert_eq!(
        count(&f, &s, ctx(&s), &query()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![Reply {
        header: Some("12".into()),
        ..reply(json!({}))
    }]);
    let body = serde_json::to_vec(&json!({"subsonic-response":{"status":"ok","version":"1.16.1"}}))
        .unwrap();
    c = ctx(&s);
    c.limits.response_bytes = body.len() + 1;
    assert_eq!(
        count(&f, &s, c, &query()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![Reply {
        header: Some("12".into()),
        ..reply(json!({}))
    }]);
    c.limits.response_bytes = body.len() + 2;
    assert_eq!(count(&f, &s, c, &query()), Ok(12));
    let f = fake(vec![]);
    let mut q = query();
    q.filter.artist_ids = Some(vec![]);
    assert_eq!(count(&f, &s, ctx(&s), &q), Ok(0));
    assert!(f.urls.borrow().is_empty());
    for which in 0..3 {
        let f = fake(vec![]);
        let mut q = query();
        match which {
            0 => q.filter.artist_ids = Some(vec![ArtistId::new("a"); 2]),
            1 => q.filter.genre_ids = Some(vec![GenreId::new("g"); 2]),
            _ => q.filter.music_folder_ids = Some(vec![FolderId::new("f"); 2]),
        };
        c = ctx(&s);
        c.limits.items = 1;
        assert_eq!(
            count(&f, &s, c, &q),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        assert!(f.urls.borrow().is_empty());
    }
    let f = fake(vec![]);
    let mut q = query();
    q.search_term = Some("x".repeat(8193));
    assert_eq!(
        count(&f, &s, ctx(&s), &q),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert!(f.urls.borrow().is_empty());
}

fn branches() -> Vec<AlbumListQuery> {
    let general = query();
    let mut search = query();
    search.search_term = Some("x".into());
    let mut favorite = query();
    favorite.filter.favorite = Some(true);
    let mut artists = query();
    artists.filter.artist_ids = Some(vec![ArtistId::new("a")]);
    vec![general, search, favorite, artists]
}
fn empty_body() -> Value {
    json!({"albumList2":{"album":[]},"searchResult3":{"album":[]},"starred":{"album":[]},"artist":{"album":[]}})
}
#[test]
fn every_branch_validates_scoped_inputs_errors_and_no_partial_totals() {
    let s = state();
    for q in branches() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            let mut invalid = q.clone();
            invalid.server = ServerId::new(server);
            assert_eq!(
                count(&f, &s, ctx(&s), &invalid),
                Err(ExchangeError::Malformed)
            );
            assert!(f.urls.borrow().is_empty());
        }
        let f = fake(vec![]);
        let mut c = ctx(&s);
        c.limits.items = 0;
        assert_eq!(
            count(&f, &s, c, &q),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        assert!(f.urls.borrow().is_empty());
        for request in [true, false] {
            let f = fake(vec![reply(empty_body())]);
            let mut c = ctx(&s);
            if request {
                c.limits.request_bytes = 1;
            } else {
                c.limits.response_bytes = 1;
            }
            assert_eq!(
                count(&f, &s, c, &q),
                Err(ExchangeError::Context(DomainError::Capacity))
            );
        }
        let f = fake(vec![Reply {
            stale: true,
            ..reply(empty_body())
        }]);
        assert_eq!(count(&f, &s, ctx(&s), &q), Err(ExchangeError::Stale));
        let f = fake(vec![Reply {
            error: Some(TransportError::Network),
            ..reply(empty_body())
        }]);
        assert_eq!(
            count(&f, &s, ctx(&s), &q),
            Err(ExchangeError::Transport(TransportError::Network))
        );
        let f = fake(vec![reply(
            json!({"status":"failed","error":{"code":40,"message":"private-marker"}}),
        )]);
        let e = count(&f, &s, ctx(&s), &q).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    let f = fake(vec![
        reply(json!({"albumList2":{"album":vec![Value::Null;500]}})),
        Reply {
            stale: true,
            ..reply(empty_body())
        },
    ]);
    assert_eq!(count(&f, &s, ctx(&s), &query()), Err(ExchangeError::Stale));
    assert_eq!(f.urls.borrow().len(), 2);
    let mut q = query();
    q.search_term = Some("x".into());
    let f = fake(vec![
        reply(json!({"searchResult3":{"album":vec![Value::Null;500]}})),
        Reply {
            error: Some(TransportError::Network),
            ..reply(empty_body())
        },
    ]);
    assert_eq!(
        count(&f, &s, ctx(&s), &q),
        Err(ExchangeError::Transport(TransportError::Network))
    );
}
#[test]
fn pending_between_page_and_final_context_guards() {
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
    }
    impl Clock for Late<'_> {
        fn now(&self) -> Milliseconds {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n > 5 {
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
            let mut future = Box::pin(get_album_count(
                &Pending,
                &s,
                ctx(&s),
                &base,
                &auth,
                &q,
                2026,
            ));
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
            for invalid in [false, true] {
                let s = state();
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                };
                let f = fake(vec![reply(if invalid {
                    json!({"albumList2":3,"searchResult3":3,"artist":3,"starred":3})
                } else {
                    empty_body()
                })]);
                assert_eq!(
                    count(&f, &clock, ctx(&s), &q),
                    Err(ExchangeError::Context(if cancel {
                        DomainError::Cancelled
                    } else {
                        DomainError::Deadline
                    }))
                );
            }
        }
    }
    struct CancelSecond<'a> {
        inner: Fake,
        state: &'a State,
        calls: Cell<u64>,
    }
    impl ProtocolTransport for CancelSecond<'_> {
        fn exchange<'a>(
            &'a self,
            request: PlannedRequest,
            context: OperationContext<'a>,
        ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n == 2 {
                self.state.cancelled.set(true);
            }
            self.inner.exchange(request, context)
        }
    }
    let s = state();
    let f = CancelSecond {
        inner: fake(vec![
            reply(json!({"albumList2":{"album":vec![Value::Null;500]}})),
            reply(empty_body()),
        ]),
        state: &s,
        calls: Cell::new(0),
    };
    assert_eq!(
        count(&f, &s, ctx(&s), &query()),
        Err(ExchangeError::Context(DomainError::Cancelled))
    );
    assert_eq!(f.calls.get(), 2);
    for cancel in [false, true] {
        let s = state();
        let clock = Late {
            calls: Cell::new(0),
            state: &s,
            cancel,
        };
        let f = fake(vec![Reply {
            header: Some("private-marker".into()),
            ..reply(empty_body())
        }]);
        assert_eq!(
            count(&f, &clock, ctx(&s), &query()),
            Err(ExchangeError::Context(if cancel {
                DomainError::Cancelled
            } else {
                DomainError::Deadline
            }))
        );
    }
}
