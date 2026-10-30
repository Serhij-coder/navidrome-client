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
async fn call(
    kind: &str,
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    q: &Value,
    body: &Value,
) -> Result<Value, MutationFailure> {
    let base = ServerBase::parse("http://fixture.invalid/music").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    let server = ServerId::new(server);
    let playlist = Scoped {
        server: server.clone(),
        id: PlaylistId::new(q["id"].as_str().or(body["id"].as_str()).unwrap_or("p")),
    };
    let receipt=match kind{
  "createPlaylist"=>return create_playlist(f,clock,c,&base,&auth,&server,body["name"].as_str().unwrap()).await.map(|r|{assert_eq!(r.value.identity.server,server);json!({"acknowledged":r.receipt.acknowledged_requests,"playlist":{"id":r.value.identity.id.as_str(),"name":r.value.name}})}),
  "updatePlaylist"=>update_playlist(f,clock,c,&base,&auth,&PlaylistMetadataMutation{playlist,name:body["name"].as_str().unwrap().into(),comment:body["comment"].as_str().map(str::to_owned),public:body["public"].as_bool()}).await,
  "deletePlaylist"=>delete_playlist(f,clock,c,&base,&auth,&playlist).await,
  "addToPlaylist"|"setPlaylistSongs"=>{
   let songs=if kind=="addToPlaylist"{&body["songId"]}else{&body["songIds"]};let query=PlaylistSongsMutation{playlist,songs:songs.as_array().unwrap().iter().map(|v|SongId::new(v.as_str().unwrap())).collect()};
   if kind=="addToPlaylist"{add_to_playlist(f,clock,c,&base,&auth,&query).await}else{set_playlist_songs(f,clock,c,&base,&auth,&query).await}
  },
  "removeFromPlaylist"=>remove_from_playlist(f,clock,c,&base,&auth,&PlaylistPositionsMutation{playlist,positions:q["songId"].as_array().unwrap().iter().map(|v|PlaylistIndex(v.as_str().unwrap().parse().unwrap())).collect()}).await,
  _=>panic!("fixture operation"),
 };
    receipt.map(|r| json!({"acknowledged":r.acknowledged_requests}))
}
#[test]
fn source_playlist_requests_optional_fields_order_and_acknowledged_values() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/p03/subsonic-playlist-mutations.json"
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
                    ..reply(v["body"].clone())
                })
                .collect(),
        );
        let kind = case["kind"].as_str().unwrap();
        let result = run(call(
            kind,
            &f,
            &s,
            ctx(&s),
            "server-a",
            &case["query"],
            &case["body"],
        ));
        if let Some(error) = case["target_error"].as_str() {
            let e = result.unwrap_err();
            assert_eq!(
                e.acknowledged_requests as u64,
                case["target_acknowledged"].as_u64().unwrap()
            );
            assert_eq!(e.outcome_unknown, case["target_unknown"].as_bool().unwrap());
            match error {
                "Malformed" => assert_eq!(e.cause, ExchangeError::Malformed),
                "Http" => assert!(matches!(e.cause, ExchangeError::Http(_))),
                "Protocol" => assert_eq!(
                    e.cause,
                    ExchangeError::Protocol(ProtocolError::Authentication)
                ),
                _ => panic!("fixture error"),
            }
        } else if kind == "createPlaylist" {
            assert_eq!(
                result.unwrap(),
                json!({"acknowledged":1,"playlist":case["expected"]})
            );
        } else {
            assert_eq!(case["expected"], Value::Null);
            assert_eq!(result.unwrap(), json!({"acknowledged":1}));
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
                .filter(|(_, v)| !v.as_array().is_some_and(Vec::is_empty))
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
#[derive(Default)]
struct Stored {
    name: String,
    comment: String,
    public: bool,
    songs: Vec<String>,
}
struct Owned {
    playlists: RefCell<std::collections::BTreeMap<String, Stored>>,
    calls: Cell<usize>,
    fail_after_apply: bool,
    malformed_created: bool,
}
fn owned() -> Owned {
    Owned {
        playlists: RefCell::new(std::collections::BTreeMap::new()),
        calls: Cell::new(0),
        fail_after_apply: false,
        malformed_created: false,
    }
}
impl ProtocolTransport for Owned {
    fn exchange<'a>(
        &'a self,
        request: PlannedRequest,
        c: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            self.calls.set(self.calls.get() + 1);
            let url = url::Url::parse(request.expose_url_for_transport()).unwrap();
            let mut params = std::collections::BTreeMap::<String, Vec<String>>::new();
            for (k, v) in url.query_pairs() {
                params
                    .entry(k.into_owned())
                    .or_default()
                    .push(v.into_owned());
            }
            let first = |key: &str| params.get(key).and_then(|v| v.first()).map(String::as_str);
            let mut response = json!({});
            let mut playlists = self.playlists.borrow_mut();
            if url.path().ends_with("createPlaylist.view") {
                if let Some(id) = first("playlistId") {
                    let p = playlists.get_mut(id).unwrap();
                    p.songs = params.get("songId").cloned().unwrap_or_default();
                } else {
                    let name = first("name").unwrap();
                    playlists.insert(
                        "new".into(),
                        Stored {
                            name: name.into(),
                            ..Stored::default()
                        },
                    );
                    response = if self.malformed_created {
                        json!({"playlist":{}})
                    } else {
                        json!({"playlist":{"id":"new","name":name}})
                    };
                }
            } else if url.path().ends_with("updatePlaylist.view") {
                let p = playlists.get_mut(first("playlistId").unwrap()).unwrap();
                if let Some(name) = first("name") {
                    p.name = name.into();
                }
                if let Some(comment) = first("comment") {
                    p.comment = comment.into();
                }
                if let Some(public) = first("public") {
                    p.public = public.parse().unwrap();
                }
                if let Some(songs) = params.get("songIdToAdd") {
                    p.songs.extend(songs.iter().cloned());
                }
                if let Some(indices) = params.get("songIndexToRemove") {
                    for i in indices {
                        p.songs.remove(i.parse::<usize>().unwrap());
                    }
                }
            } else if url.path().ends_with("deletePlaylist.view") {
                playlists.remove(first("id").unwrap());
            } else {
                panic!("disposable route");
            }
            drop(playlists);
            if self.fail_after_apply {
                return Err(TransportError::Network);
            }
            let obj = response.as_object_mut().unwrap();
            obj.insert("status".into(), json!("ok"));
            obj.insert("version".into(), json!("1.16.1"));
            Ok(TransportResponse {
                correlation: c.correlation,
                status: 200,
                body: serde_json::to_vec(&json!({"subsonic-response":response})).unwrap(),
                total_count_header: None,
            })
        })
    }
}
#[test]
fn disposable_playlist_workflow_preserves_duplicate_songs_positions_and_metadata() {
    let s = state();
    let f = owned();
    let created = run(call(
        "createPlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({}),
        &json!({"name":"Original"}),
    ))
    .unwrap();
    assert_eq!(created["playlist"]["id"], "new");
    run(call(
        "setPlaylistSongs",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({}),
        &json!({"id":"new","songIds":["same","other","same"]}),
    ))
    .unwrap();
    assert_eq!(f.playlists.borrow()["new"].songs, ["same", "other", "same"]);
    run(call(
        "removeFromPlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({"id":"new","songId":["2","0"]}),
        &json!({}),
    ))
    .unwrap();
    assert_eq!(f.playlists.borrow()["new"].songs, ["other"]);
    run(call(
        "addToPlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({"id":"new"}),
        &json!({"songId":["same","same"]}),
    ))
    .unwrap();
    assert_eq!(f.playlists.borrow()["new"].songs, ["other", "same", "same"]);
    run(call(
        "updatePlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({"id":"new"}),
        &json!({"name":"Renamed","comment":"","public":false}),
    ))
    .unwrap();
    assert_eq!(f.playlists.borrow()["new"].name, "Renamed");
    assert!(!f.playlists.borrow()["new"].public);
    assert_eq!(f.playlists.borrow()["new"].songs, ["other", "same", "same"]);
    run(call(
        "deletePlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({"id":"new"}),
        &json!({}),
    ))
    .unwrap();
    assert!(f.playlists.borrow().is_empty());
    assert_eq!(f.calls.get(), 6);
    let mut f = owned();
    f.fail_after_apply = true;
    let e = run(call(
        "createPlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({}),
        &json!({"name":"Uncertain"}),
    ))
    .unwrap_err();
    assert_eq!(
        e,
        MutationFailure {
            acknowledged_requests: 0,
            outcome_unknown: true,
            cause: ExchangeError::Transport(TransportError::Network)
        }
    );
    assert!(f.playlists.borrow().contains_key("new"));
    assert_eq!(f.calls.get(), 1);
    let mut f = owned();
    f.malformed_created = true;
    let e = run(call(
        "createPlaylist",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({}),
        &json!({"name":"Created without usable metadata"}),
    ))
    .unwrap_err();
    assert_eq!(
        e,
        MutationFailure {
            acknowledged_requests: 1,
            outcome_unknown: false,
            cause: ExchangeError::Malformed
        }
    );
    assert!(f.playlists.borrow().contains_key("new"));
    assert_eq!(f.calls.get(), 1);
}
fn cases() -> Vec<(&'static str, Value, Value)> {
    vec![
        ("createPlaylist", json!({}), json!({"name":"N"})),
        ("updatePlaylist", json!({"id":"p"}), json!({"name":"N"})),
        ("deletePlaylist", json!({"id":"p"}), json!({})),
        ("addToPlaylist", json!({"id":"p"}), json!({"songId":["a"]})),
        (
            "setPlaylistSongs",
            json!({}),
            json!({"id":"p","songIds":["a"]}),
        ),
        (
            "removeFromPlaylist",
            json!({"id":"p","songId":["0"]}),
            json!({}),
        ),
    ]
}
#[test]
fn bounded_inputs_and_failed_exchanges_never_report_unacknowledged_success() {
    let s = state();
    for (kind, q, body) in cases() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            let e = run(call(kind, &f, &s, ctx(&s), &server, &q, &body)).unwrap_err();
            assert_eq!(e.cause, ExchangeError::Malformed);
            assert!(!e.outcome_unknown);
            assert_eq!(e.acknowledged_requests, 0);
            assert!(f.urls.borrow().is_empty());
        }
        let f = fake(vec![]);
        let mut c = ctx(&s);
        c.limits.request_bytes = 1;
        let e = run(call(kind, &f, &s, c, "s", &q, &body)).unwrap_err();
        assert_eq!(e.cause, ExchangeError::Context(DomainError::Capacity));
        assert!(!e.outcome_unknown);
        assert!(f.urls.borrow().is_empty());
        let f = fake(vec![reply(json!({}))]);
        c = ctx(&s);
        c.limits.response_bytes = 1;
        let e = run(call(kind, &f, &s, c, "s", &q, &body)).unwrap_err();
        assert!(e.outcome_unknown);
        assert_eq!(e.acknowledged_requests, 0);
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
            let e = run(call(kind, &f, &s, ctx(&s), "s", &q, &body)).unwrap_err();
            assert_eq!(e.acknowledged_requests, 0);
            assert!(e.outcome_unknown);
            assert!(!format!("{e:?}").contains("private-marker"));
        }
    }
    let base = ServerBase::parse("http://fixture.invalid").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    let playlist = Scoped {
        server: ServerId::new("s"),
        id: PlaylistId::new("p"),
    };
    let f = fake(vec![]);
    let q = PlaylistPositionsMutation {
        playlist: playlist.clone(),
        positions: vec![PlaylistIndex(9007199254740992)],
    };
    let e = run(remove_from_playlist(&f, &s, ctx(&s), &base, &auth, &q)).unwrap_err();
    assert_eq!(e.cause, ExchangeError::Malformed);
    assert!(!e.outcome_unknown);
    assert!(f.urls.borrow().is_empty());
    for n in [2, 128] {
        let f = fake(vec![]);
        let mut c = ctx(&s);
        if n == 2 {
            c.limits.items = 1;
        }
        let q = PlaylistSongsMutation {
            playlist: playlist.clone(),
            songs: vec![SongId::new("same"); n],
        };
        let e = run(add_to_playlist(&f, &s, c, &base, &auth, &q)).unwrap_err();
        assert!(!e.outcome_unknown);
        assert_eq!(e.acknowledged_requests, 0);
        assert!(f.urls.borrow().is_empty());
    }
}
#[test]
fn pending_and_final_context_retain_known_acknowledgements_and_unknown_requests() {
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
    for (kind, q, body) in cases() {
        for cancel in [false, true] {
            let s = state();
            let mut future = Box::pin(call(kind, &Pending, &s, ctx(&s), "s", &q, &body));
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
            for malformed in [false, true] {
                let s = state();
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                    after: 6,
                };
                let f = fake(vec![reply(if malformed {
                    json!({"playlist":3})
                } else {
                    json!({"playlist":{"id":"new","name":"N"}})
                })]);
                let e = run(call(kind, &f, &clock, ctx(&s), "s", &q, &body)).unwrap_err();
                assert_eq!(
                    e,
                    MutationFailure {
                        acknowledged_requests: 1,
                        outcome_unknown: false,
                        cause
                    }
                );
            }
        }
    }
}
