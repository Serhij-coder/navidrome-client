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
fn entity(kind: &str, id: &str) -> LibraryEntityId {
    match kind {
        "album" => LibraryEntityId::Album(AlbumId::new(id)),
        "albumArtist" => LibraryEntityId::AlbumArtist(ArtistId::new(id)),
        "artist" => LibraryEntityId::Artist(ArtistId::new(id)),
        "folder" => LibraryEntityId::Folder(FolderId::new(id)),
        "genre" => LibraryEntityId::Genre(GenreId::new(id)),
        "playlist" => LibraryEntityId::Playlist(PlaylistId::new(id)),
        "playlistSong" => LibraryEntityId::PlaylistSong(SongId::new(id)),
        "queueSong" => LibraryEntityId::QueueSong(SongId::new(id)),
        "radioStation" => LibraryEntityId::RadioStation(RadioStationId::new(id)),
        "song" => LibraryEntityId::Song(SongId::new(id)),
        _ => panic!("fixture identity family"),
    }
}
fn favorite(q: &Value, server: ServerId) -> FavoriteMutation {
    let ids: Vec<_> = q["id"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    FavoriteMutation {
        server,
        target: match q["type"].as_str().unwrap() {
            "album" => FavoriteTarget::Albums(ids.into_iter().map(AlbumId::new).collect()),
            "albumArtist" => {
                FavoriteTarget::AlbumArtists(ids.into_iter().map(ArtistId::new).collect())
            }
            "artist" => FavoriteTarget::Artists(ids.into_iter().map(ArtistId::new).collect()),
            "song" => FavoriteTarget::Songs(ids.into_iter().map(SongId::new).collect()),
            "playlistSong" => {
                FavoriteTarget::PlaylistSongs(ids.into_iter().map(SongId::new).collect())
            }
            _ => panic!("fixture favorite family"),
        },
    }
}
async fn call(
    kind: &str,
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    q: &Value,
) -> Result<Value, MutationFailure> {
    let base = ServerBase::parse("http://fixture.invalid/music").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    let server = ServerId::new(server);
    let result = match kind {
        "createFavorite" => create_favorite(f, clock, c, &base, &auth, &favorite(q, server)).await,
        "deleteFavorite" => delete_favorite(f, clock, c, &base, &auth, &favorite(q, server)).await,
        "setRating" => {
            set_rating(
                f,
                clock,
                c,
                &base,
                &auth,
                &RatingMutation {
                    server,
                    items: q["id"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| entity(q["type"].as_str().unwrap(), v.as_str().unwrap()))
                        .collect(),
                    rating: q["rating"].as_f64().unwrap(),
                },
            )
            .await
        }
        "startLibraryScan" => start_library_scan(f, clock, c, &base, &auth, &server).await,
        "refreshItems" => refresh_items(f, clock, c, &base, &auth, &server).await,
        "getUserInfo" => {
            return get_user_info(
                f,
                clock,
                c,
                &base,
                &auth,
                &server,
                q["username"].as_str().unwrap(),
            )
            .await
            .map(|u| {
                assert_eq!(u.server, server);
                assert_eq!(u.created_at, None);
                assert_eq!(u.email, None);
                assert_eq!(u.last_login_at, None);
                assert_eq!(u.updated_at, None);
                json!({"id":u.id.as_str(),"name":u.name,"isAdmin":u.is_admin})
            })
            .map_err(|cause| MutationFailure {
                acknowledged_requests: 0,
                outcome_unknown: false,
                cause,
            });
        }
        _ => panic!("fixture operation"),
    };
    result.map(|r| json!({"acknowledged":r.acknowledged_requests}))
}
#[test]
fn source_requests_supported_families_and_explicit_failure_differences() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/p03/subsonic-basic-mutations.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let f = fake(
            case["responses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Reply {
                    status: v["status"].as_u64().unwrap_or(200).try_into().unwrap(),
                    error: v.get("throw").map(|_| TransportError::Network),
                    ..reply(v.get("body").cloned().unwrap_or(json!({})))
                })
                .collect(),
        );
        let kind = case["kind"].as_str().unwrap();
        let result = run(call(kind, &f, &s, ctx(&s), "server-a", &case["query"]));
        if let Some(error) = case["target_error"].as_str() {
            let e = result.unwrap_err();
            assert_eq!(
                e.acknowledged_requests as u64,
                case["target_acknowledged"].as_u64().unwrap_or(0),
                "{}",
                case["name"]
            );
            assert_eq!(
                e.outcome_unknown,
                case["target_unknown"].as_bool().unwrap_or(false),
                "{}",
                case["name"]
            );
            match error {
                "Malformed" => assert_eq!(e.cause, ExchangeError::Malformed),
                "Http" => assert!(matches!(e.cause, ExchangeError::Http(_))),
                "Protocol" => assert_eq!(
                    e.cause,
                    ExchangeError::Protocol(ProtocolError::Authentication)
                ),
                "Transport" => {
                    assert_eq!(e.cause, ExchangeError::Transport(TransportError::Network))
                }
                _ => panic!("fixture error"),
            }
        } else if kind == "getUserInfo" {
            assert_eq!(result.unwrap(), case["expected"]);
        } else {
            assert_eq!(case["expected"], Value::Null);
            assert_eq!(
                result.unwrap(),
                json!({"acknowledged":case["target_acknowledged"]})
            );
        }
        let urls = f.urls.borrow();
        let requests = case["requests"].as_array().unwrap();
        let wanted = case["target_request_count"]
            .as_u64()
            .map(|v| v as usize)
            .unwrap_or(requests.len());
        assert_eq!(urls.len(), wanted, "{}", case["name"]);
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
            let mut actual = std::collections::BTreeMap::<String, Vec<String>>::new();
            for (k, v) in url
                .query_pairs()
                .filter(|(k, _)| !["c", "f", "v", "u", "p"].contains(&k.as_ref()))
            {
                actual
                    .entry(k.into_owned())
                    .or_default()
                    .push(v.into_owned());
            }
            let expected: std::collections::BTreeMap<_, _> = request["args"]["query"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    let string = |v: &Value| {
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    };
                    (
                        k.clone(),
                        if let Some(v) = v.as_array() {
                            v.iter().map(string).collect()
                        } else {
                            vec![string(v)]
                        },
                    )
                })
                .collect();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}
struct OwnedRatings {
    applied: RefCell<Vec<(String, f64)>>,
    calls: Cell<usize>,
    fail_after_apply: Option<usize>,
}
impl ProtocolTransport for OwnedRatings {
    fn exchange<'a>(
        &'a self,
        request: PlannedRequest,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            self.calls.set(self.calls.get() + 1);
            let url = url::Url::parse(request.expose_url_for_transport()).unwrap();
            assert!(url.path().ends_with("setRating.view"));
            let params: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
            self.applied
                .borrow_mut()
                .push((params["id"].to_string(), params["rating"].parse().unwrap()));
            if self.fail_after_apply == Some(self.calls.get()) {
                return Err(TransportError::Network);
            }
            Ok(TransportResponse {
                correlation: c.correlation,
                status: 200,
                body: br#"{"subsonic-response":{"status":"ok","version":"1.16.1"}}"#.to_vec(),
                total_count_header: None,
            })
        })
    }
}
fn owned(fail_after_apply: Option<usize>) -> OwnedRatings {
    OwnedRatings {
        applied: RefCell::new(vec![]),
        calls: Cell::new(0),
        fail_after_apply,
    }
}
#[test]
fn disposable_state_proves_order_repetition_uncertainty_and_preflight_before_effects() {
    let s = state();
    let f = owned(Some(2));
    let q = json!({"type":"song","id":["a","b","c"],"rating":0.5});
    let e = run(call("setRating", &f, &s, ctx(&s), "s", &q)).unwrap_err();
    assert_eq!(
        e,
        MutationFailure {
            acknowledged_requests: 1,
            outcome_unknown: true,
            cause: ExchangeError::Transport(TransportError::Network)
        }
    );
    assert_eq!(
        *f.applied.borrow(),
        vec![("a".into(), 0.5), ("b".into(), 0.5)]
    );
    assert_eq!(f.calls.get(), 2);
    let f = owned(None);
    let q = json!({"type":"song","id":["a","a","b"],"rating":5});
    assert_eq!(
        run(call("setRating", &f, &s, ctx(&s), "s", &q)),
        Ok(json!({"acknowledged":3}))
    );
    assert_eq!(
        *f.applied.borrow(),
        vec![("a".into(), 5.0), ("a".into(), 5.0), ("b".into(), 5.0)]
    );
    for bad in ["".to_owned(), "x".repeat(8193)] {
        let f = owned(None);
        let q = json!({"type":"song","id":["a",bad],"rating":3});
        let e = run(call("setRating", &f, &s, ctx(&s), "s", &q)).unwrap_err();
        assert_eq!(e.acknowledged_requests, 0);
        assert!(!e.outcome_unknown);
        assert_eq!(f.calls.get(), 0);
        assert!(f.applied.borrow().is_empty());
    }
    let f = owned(None);
    let q = json!({"type":"song","id":["a","longer"],"rating":3});
    let mut c = ctx(&s);
    c.limits.request_bytes = 1;
    let e = run(call("setRating", &f, &s, c, "s", &q)).unwrap_err();
    assert_eq!(e.cause, ExchangeError::Context(DomainError::Capacity));
    assert!(!e.outcome_unknown);
    assert_eq!(f.calls.get(), 0);
    let f = owned(None);
    let base = ServerBase::parse("http://fixture.invalid/music").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    let q = RatingMutation {
        server: ServerId::new("s"),
        items: vec![LibraryEntityId::Song(SongId::new("a"))],
        rating: -0.0,
    };
    assert_eq!(
        run(set_rating(&f, &s, ctx(&s), &base, &auth, &q)),
        Ok(MutationReceipt {
            acknowledged_requests: 1
        })
    );
    assert_eq!(f.applied.borrow()[0].1, 0.0);
}
fn mutation_cases() -> Vec<(&'static str, Value)> {
    vec![
        ("createFavorite", json!({"type":"song","id":["a"]})),
        ("deleteFavorite", json!({"type":"song","id":["a"]})),
        ("setRating", json!({"type":"song","id":["a"],"rating":3})),
        ("startLibraryScan", json!({})),
        ("refreshItems", json!({})),
    ]
}
#[test]
fn bounded_scoped_mutations_distinguish_pre_dispatch_and_unknown_failures() {
    let s = state();
    for (kind, q) in mutation_cases() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            let e = run(call(kind, &f, &s, ctx(&s), &server, &q)).unwrap_err();
            assert_eq!(e.cause, ExchangeError::Malformed);
            assert_eq!(e.acknowledged_requests, 0);
            assert!(!e.outcome_unknown);
            assert!(f.urls.borrow().is_empty());
        }
        let f = fake(vec![]);
        let mut c = ctx(&s);
        c.limits.request_bytes = 1;
        let e = run(call(kind, &f, &s, c, "s", &q)).unwrap_err();
        assert_eq!(e.cause, ExchangeError::Context(DomainError::Capacity));
        assert!(!e.outcome_unknown);
        assert!(f.urls.borrow().is_empty());
        for response in [
            Reply {
                stale: true,
                ..reply(json!({}))
            },
            Reply {
                error: Some(TransportError::Network),
                ..reply(json!({}))
            },
            reply(json!({"status":"failed","error":{"code":40,"message":"private-marker"}})),
        ] {
            let f = fake(vec![response]);
            let e = run(call(kind, &f, &s, ctx(&s), "s", &q)).unwrap_err();
            assert_eq!(e.acknowledged_requests, 0);
            assert!(e.outcome_unknown);
            assert_eq!(f.urls.borrow().len(), 1);
            assert!(!format!("{e:?}").contains("private-marker"));
        }
        let f = fake(vec![reply(json!({}))]);
        c = ctx(&s);
        c.limits.response_bytes = 1;
        let e = run(call(kind, &f, &s, c, "s", &q)).unwrap_err();
        assert!(e.outcome_unknown);
        assert_eq!(e.cause, ExchangeError::Context(DomainError::Capacity));
    }
    let base = ServerBase::parse("http://fixture.invalid").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    for rating in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let f = owned(None);
        let q = RatingMutation {
            server: ServerId::new("s"),
            items: vec![LibraryEntityId::Song(SongId::new("a"))],
            rating,
        };
        assert_eq!(
            run(set_rating(&f, &s, ctx(&s), &base, &auth, &q))
                .unwrap_err()
                .cause,
            ExchangeError::Malformed
        );
        assert_eq!(f.calls.get(), 0);
    }
    let f = fake(vec![]);
    let q = json!({"type":"song","id":["a","b"],"rating":3});
    let mut c = ctx(&s);
    c.limits.items = 1;
    assert_eq!(
        run(call("setRating", &f, &s, c, "s", &q))
            .unwrap_err()
            .cause,
        ExchangeError::Context(DomainError::Capacity)
    );
    assert!(f.urls.borrow().is_empty());
    let f = fake(vec![]);
    let q = json!({"type":"song","id":vec!["a";129]});
    let e = run(call("createFavorite", &f, &s, ctx(&s), "s", &q)).unwrap_err();
    assert!(!e.outcome_unknown);
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn mutation_pending_between_requests_and_final_context_preserve_progress() {
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
    for (kind, q) in mutation_cases() {
        for cancel in [false, true] {
            let s = state();
            let mut future = Box::pin(call(kind, &Pending, &s, ctx(&s), "s", &q));
            let w = Waker::from(Arc::new(Noop));
            let mut cx = Context::from_waker(&w);
            assert!(future.as_mut().poll(&mut cx).is_pending());
            if cancel {
                s.cancelled.set(true);
            } else {
                s.now.set(11);
            }
            let cause = ExchangeError::Context(if cancel {
                DomainError::Cancelled
            } else {
                DomainError::Deadline
            });
            assert_eq!(
                future.as_mut().poll(&mut cx),
                Poll::Ready(Err(MutationFailure {
                    acknowledged_requests: 0,
                    outcome_unknown: true,
                    cause
                }))
            );
            drop(future);
            let s = state();
            let clock = Late {
                calls: Cell::new(0),
                state: &s,
                cancel,
                after: if kind == "setRating" { 7 } else { 6 },
            };
            let f = fake(vec![reply(json!({}))]);
            let e = run(call(kind, &f, &clock, ctx(&s), "s", &q)).unwrap_err();
            assert_eq!(
                e,
                MutationFailure {
                    acknowledged_requests: 1,
                    outcome_unknown: false,
                    cause
                }
            );
            for protocol in [false, true] {
                let s = state();
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                    after: (if protocol { 6 } else { 5 }) + u64::from(kind == "setRating"),
                };
                let f = fake(vec![if protocol {
                    reply(json!({"status":"failed","error":{"code":40,"message":"private-marker"}}))
                } else {
                    Reply {
                        status: 403,
                        ..reply(json!({}))
                    }
                }]);
                let e = run(call(kind, &f, &clock, ctx(&s), "s", &q)).unwrap_err();
                assert_eq!(
                    e,
                    MutationFailure {
                        acknowledged_requests: 0,
                        outcome_unknown: true,
                        cause
                    }
                );
            }
        }
    }
    let s = state();
    let clock = Late {
        calls: Cell::new(0),
        state: &s,
        cancel: false,
        after: 8,
    };
    let f = owned(None);
    let q = json!({"type":"song","id":["a","b"],"rating":3});
    let e = run(call("setRating", &f, &clock, ctx(&s), "s", &q)).unwrap_err();
    assert_eq!(
        e,
        MutationFailure {
            acknowledged_requests: 1,
            outcome_unknown: false,
            cause: ExchangeError::Context(DomainError::Deadline)
        }
    );
    assert_eq!(f.calls.get(), 1);
    let s = state();
    let clock = Late {
        calls: Cell::new(0),
        state: &s,
        cancel: false,
        after: 1,
    };
    let f = owned(None);
    let q = json!({"type":"song","id":[],"rating":3});
    let e = run(call("setRating", &f, &clock, ctx(&s), "s", &q)).unwrap_err();
    assert_eq!(
        e,
        MutationFailure {
            acknowledged_requests: 0,
            outcome_unknown: false,
            cause: ExchangeError::Context(DomainError::Deadline)
        }
    );
    assert_eq!(f.calls.get(), 0);
}

#[test]
fn user_target_name_is_data_with_protected_auth_and_checked_role_types() {
    let s = state();
    let f = fake(vec![reply(
        json!({"user":{"username":"canonical","adminRole":true,"email":"ignored"}}),
    )]);
    let base = ServerBase::parse("http://fixture.invalid/music").unwrap();
    let auth = Auth::legacy("principal", "dummy-secret").unwrap();
    let server = ServerId::new("other");
    let target = "target&u=override%名";
    let user = run(get_user_info(
        &f,
        &s,
        ctx(&s),
        &base,
        &auth,
        &server,
        target,
    ))
    .unwrap();
    assert_eq!(user.server, server);
    assert_eq!(user.name, "canonical");
    assert_eq!(user.id.as_str(), "canonical");
    assert_eq!(user.email, None);
    let url = url::Url::parse(&f.urls.borrow()[0]).unwrap();
    let params: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
    assert_eq!(params["u"], "principal");
    assert_eq!(params["username"], target);
    assert_eq!(params.len(), 6);
    assert!(matches!(
        base.request(&auth, "getUser.view", &[("username", target)]),
        Err(Error::InvalidQuery)
    ));
    for bad_server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
        let f = fake(vec![]);
        assert_eq!(
            run(get_user_info(
                &f,
                &s,
                ctx(&s),
                &base,
                &auth,
                &ServerId::new(bad_server),
                "name"
            )),
            Err(ExchangeError::Malformed)
        );
        assert!(f.urls.borrow().is_empty());
    }
    let f = fake(vec![]);
    assert!(
        run(get_user_info(
            &f,
            &s,
            ctx(&s),
            &base,
            &auth,
            &server,
            &"x".repeat(8193)
        ))
        .is_err()
    );
    assert!(f.urls.borrow().is_empty());
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
    struct Late {
        calls: Cell<u64>,
    }
    impl Clock for Late {
        fn now(&self) -> Milliseconds {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            Milliseconds(if n > 5 { 11 } else { 10 })
        }
    }
    let s = state();
    let mut future = Box::pin(get_user_info(
        &Pending,
        &s,
        ctx(&s),
        &base,
        &auth,
        &server,
        "name",
    ));
    let w = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&w);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    s.cancelled.set(true);
    assert_eq!(
        future.as_mut().poll(&mut cx),
        Poll::Ready(Err(ExchangeError::Context(DomainError::Cancelled)))
    );
    drop(future);
    for body in [
        json!({"user":{"username":"u","adminRole":false}}),
        json!({"user":3}),
    ] {
        let s = state();
        let f = fake(vec![reply(body)]);
        let clock = Late {
            calls: Cell::new(0),
        };
        assert_eq!(
            run(get_user_info(
                &f,
                &clock,
                ctx(&s),
                &base,
                &auth,
                &server,
                "name"
            )),
            Err(ExchangeError::Context(DomainError::Deadline))
        );
    }
}
