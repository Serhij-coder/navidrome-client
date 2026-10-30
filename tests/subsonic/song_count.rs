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
fn count(
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    q: &SongListQuery,
) -> Result<u64, ExchangeError> {
    run(get_song_count(
        f,
        clock,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        q,
    ))
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
#[test]
fn original_controller_counts_exact_routes_parameters_and_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/p03/subsonic-song-count.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let f = fake(
            case["responses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| Reply {
                    status: r["status"].as_u64().unwrap_or(200).try_into().unwrap(),
                    header: r["header"].as_str().map(str::to_owned),
                    ..reply(r["body"].clone())
                })
                .collect(),
        );
        let result = count(&f, &s, ctx(&s), &fixture_query(&case["query"]));
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
                Err(ExchangeError::Protocol(ProtocolError::Authentication)),
                "{}",
                case["name"]
            ),
            None => assert_eq!(
                result,
                Ok(case["expected"].as_u64().unwrap()),
                "{}",
                case["name"]
            ),
            _ => panic!("unknown fixture target error"),
        }
        let requests = case["requests"].as_array().unwrap();
        let request_count = case["target_request_count"]
            .as_u64()
            .map(|v| v as usize)
            .unwrap_or(requests.len());
        let urls = f.urls.borrow();
        assert_eq!(urls.len(), request_count, "{}", case["name"]);
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
fn growing_libraries_requests_rows_and_all_selector_dimensions_are_bounded() {
    let s = state();
    let f = fake(
        (0..10)
            .map(|_| reply(json!({"searchResult3":{"song":[null]}})))
            .collect(),
    );
    let mut c = ctx(&s);
    c.limits.items = 7;
    assert_eq!(
        count(&f, &s, c, &query()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), 3);
    let mut q = query();
    q.search_term = Some("x".into());
    let f = fake(vec![]);
    // Selector validation must precede even a request on a higher-priority path.
    assert!(f.urls.borrow().is_empty());
    for which in 0..5 {
        let mut q = q.clone();
        match which {
            0 => q.filter.album_ids = Some(vec![AlbumId::new("a"); 3]),
            1 => q.filter.artist_ids = Some(vec![ArtistId::new("a"); 3]),
            2 => q.filter.album_artist_ids = Some(vec![ArtistId::new("a"); 3]),
            3 => q.filter.genre_ids = Some(vec![GenreId::new("g"); 3]),
            _ => q.filter.music_folder_ids = Some(vec![FolderId::new("f"); 3]),
        }
        c = ctx(&s);
        c.limits.items = 2;
        assert_eq!(
            count(&f, &s, c, &q),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
    }
    assert!(f.urls.borrow().is_empty());
    let f = fake(vec![
        reply(json!({"searchResult3":{"song":vec![Value::Null;500]}})),
        reply(json!({"searchResult3":{"song":vec![Value::Null;500]}})),
    ]);
    c = ctx(&s);
    c.limits.items = 1002;
    assert_eq!(
        count(&f, &s, c, &q),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), 2);
    let f = fake(vec![reply(
        json!({"artist":{"album":[{"id":"a"},{"id":"a"}]}}),
    )]);
    q = fixture_query(&json!({"artistIds":["a"]}));
    c = ctx(&s);
    c.limits.items = 3;
    assert_eq!(
        count(&f, &s, c, &q),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), 1);
    let f = fake(vec![]);
    q = fixture_query(&json!({"albumArtistIds":[]}));
    assert_eq!(count(&f, &s, ctx(&s), &q), Ok(0));
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn typed_errors_stale_replies_and_byte_limits_never_publish_partial_counts() {
    let s = state();
    for q in branches() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            let mut q = q.clone();
            q.server = ServerId::new(server);
            assert_eq!(count(&f, &s, ctx(&s), &q), Err(ExchangeError::Malformed));
            assert!(f.urls.borrow().is_empty());
        }
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
            assert_eq!(f.urls.borrow().len(), usize::from(!request));
        }
        for stale in [true, false] {
            let f = fake(vec![Reply {
                stale,
                error: (!stale).then_some(TransportError::Network),
                ..reply(empty_body())
            }]);
            assert_eq!(
                count(&f, &s, ctx(&s), &q),
                Err(if stale {
                    ExchangeError::Stale
                } else {
                    ExchangeError::Transport(TransportError::Network)
                })
            );
        }
        let f = fake(vec![reply(
            json!({"status":"failed","error":{"code":40,"message":"private-marker"}}),
        )]);
        let e = count(&f, &s, ctx(&s), &q).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    let f = fake(vec![
        reply(json!({"searchResult3":{"song":[null]}})),
        Reply {
            stale: true,
            ..reply(empty_body())
        },
    ]);
    assert_eq!(count(&f, &s, ctx(&s), &query()), Err(ExchangeError::Stale));
    let q = fixture_query(&json!({"albumIds":["a","b"]}));
    let f = fake(vec![
        reply(json!({"album":{"song":[null,null]}})),
        Reply {
            error: Some(TransportError::Network),
            ..reply(empty_body())
        },
    ]);
    assert_eq!(
        count(&f, &s, ctx(&s), &q),
        Err(ExchangeError::Transport(TransportError::Network))
    );
    let f = fake(vec![]);
    let mut q = query();
    q.search_term = Some("x".repeat(8193));
    assert_eq!(
        count(&f, &s, ctx(&s), &q),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn pending_between_request_final_and_no_io_completion_guards() {
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
            let mut future = Box::pin(get_song_count(&Pending, &s, ctx(&s), &base, &auth, &q));
            let w = Waker::from(Arc::new(Noop));
            let mut cx = Context::from_waker(&w);
            assert!(future.as_mut().poll(&mut cx).is_pending());
            if cancel {
                s.cancelled.set(true);
            } else {
                s.now.set(11);
            }
            let e = ExchangeError::Context(if cancel {
                DomainError::Cancelled
            } else {
                DomainError::Deadline
            });
            assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(Err(e)));
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
                    json!({"searchResult3":3,"songsByGenre":3,"starred":3,"album":3,"artist":3})
                } else {
                    empty_body()
                })]);
                assert_eq!(count(&f, &clock, ctx(&s), &q), Err(e));
                // General/genre need a second request, but must expire before dispatch.
                assert_eq!(f.urls.borrow().len(), 1);
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
    let q = fixture_query(&json!({"albumIds":[]}));
    assert_eq!(
        count(&f, &clock, ctx(&s), &q),
        Err(ExchangeError::Context(DomainError::Deadline))
    );
    assert!(f.urls.borrow().is_empty());
}

#[test]
fn stable_dataset_sizes_across_page_and_stride_boundaries() {
    // This server double only implements offset/size slicing of a fixed dataset;
    // it has no knowledge of the client's probing or count algorithm.
    struct Dataset {
        len: usize,
        calls: Cell<usize>,
    }
    impl ProtocolTransport for Dataset {
        fn exchange<'a>(
            &'a self,
            request: PlannedRequest,
            c: OperationContext<'a>,
        ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
            Box::pin(async move {
                self.calls.set(self.calls.get() + 1);
                let url = url::Url::parse(request.expose_url_for_transport()).unwrap();
                let pairs: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
                let genre = url.path().ends_with("getSongsByGenre.view");
                let offset = pairs
                    .get(if genre { "offset" } else { "songOffset" })
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                let size = pairs
                    .get(if genre { "count" } else { "songCount" })
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                let len = self.len.saturating_sub(offset).min(size);
                let parent = if genre {
                    "songsByGenre"
                } else {
                    "searchResult3"
                };
                Ok(TransportResponse{correlation:c.correlation,status:200,body:serde_json::to_vec(&json!({"subsonic-response":{"status":"ok","version":"1.16.1",parent:{"song":vec![Value::Null;len]}}})).unwrap(),total_count_header:None})
            })
        }
    }
    for len in [0, 1, 499, 500, 501, 4999, 5000, 5001, 9999, 10000, 10001] {
        for query_value in [
            json!({}),
            json!({"genreIds":["g"]}),
            json!({"searchTerm":"x"}),
        ] {
            let s = state();
            let dataset = Dataset {
                len,
                calls: Cell::new(0),
            };
            let mut c = ctx(&s);
            c.limits.items = 20000;
            assert_eq!(
                count(&dataset, &s, c, &fixture_query(&query_value)),
                Ok(len as u64),
                "len={len} query={query_value}"
            );
            assert!(dataset.calls.get() <= 24);
        }
    }
}
