#![recursion_limit = "256"]
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
#[path = "support/projection.rs"]
mod projections;
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
fn page(p: Page<Song>) -> Value {
    json!({"items":p.items.iter().map(projections::song).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})
}
fn field(v: &Field<String>) -> Value {
    match v {
        Field::Value(s) => json!(s),
        Field::Null => Value::Null,
        Field::Absent => panic!("source radio image fields are always present"),
    }
}
async fn call(
    kind: &str,
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    q: &Value,
    features: &Value,
) -> Result<Value, ExchangeError> {
    let base = ServerBase::parse("http://fixture.invalid/music").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    let server = ServerId::new(server);
    let artist = Scoped {
        server: server.clone(),
        id: ArtistId::new(q["artistId"].as_str().unwrap_or("a")),
    };
    match kind{
        "getFavoriteSongs"=>get_favorite_songs(f,clock,c,&base,&auth,&FavoriteSongListQuery{artist,limit:q["limit"].as_u64(),mode:if q["type"]=="rating"{FavoriteSongMode::Rating}else{FavoriteSongMode::Favorite}}).await.map(page),
        "getTopSongs"=>{
            let mut capabilities=Capabilities::default();if let Some(versions)=features["topSongsByArtistId"].as_array(){capabilities.0.insert(ServerFeature::TopSongsByArtistId,versions.iter().map(|v|v.as_u64().unwrap().try_into().unwrap()).collect());}
            get_top_songs(f,clock,c,&base,&auth,&TopSongListQuery{artist,artist_name:q["artist"].as_str().unwrap_or("").into(),limit:q["limit"].as_u64(),mode:if q["type"]=="personal"{TopSongMode::Personal}else{TopSongMode::Community}},&capabilities).await.map(page)
        },
        "getInternetRadioStations"=>get_internet_radio_stations(f,clock,c,&base,&auth,&server).await.map(|rows|json!(rows.iter().map(|r|{
            assert_eq!(r.server,server);assert_eq!(r.thumb_hash,Field::Absent);assert_eq!(r.uploaded_image,Field::Absent);
            json!({"id":r.id.as_str(),"name":r.name,"streamUrl":r.stream_url,"homepageUrl":r.homepage_url,"imageId":field(&r.image_id),"imageUrl":field(&r.image_url)})
        }).collect::<Vec<_>>())),
        "getScanStatus"=>get_scan_status(f,clock,c,&base,&auth,&server).await.map(|s|json!({"count":s.count,"folderCount":s.folder_count,"scanning":s.scanning,"lastScan":s.last_scan})),
        _=>panic!("unknown fixture operation"),
    }
}
#[test]
fn pinned_composed_song_and_service_reads_preserve_outputs_parameters_and_features() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/p03/subsonic-catalog-reads.json")).unwrap();
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
            &case["features"],
        ));
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
                let mut expected = case["expected"].clone();
                project_expected(&mut expected);
                if kind == "getScanStatus" && expected.get("lastScan").is_none() {
                    expected["lastScan"] = Value::Null;
                }
                projections::equivalent(
                    &result.unwrap_or_else(|e| panic!("{}: {e:?}", case["name"])),
                    &expected,
                );
            }
            _ => panic!("unknown target error"),
        }
        assert_eq!(case["rng_calls"], 0, "compositions never shuffle");
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
                .map(|q| {
                    q.iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                v.as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| v.to_string()),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}
fn cases() -> Vec<(&'static str, Value)> {
    vec![
        ("getFavoriteSongs", json!({"artistId":"a"})),
        ("getTopSongs", json!({"artistId":"a","artist":"A"})),
        (
            "getTopSongs",
            json!({"artistId":"a","artist":"A","type":"personal"}),
        ),
        ("getInternetRadioStations", json!({})),
        ("getScanStatus", json!({})),
    ]
}
fn empty_body() -> Value {
    json!({"artist":{"album":[]},"topSongs":{"song":[]},"internetRadioStations":{"internetRadioStation":[]},"scanStatus":{"count":0,"folderCount":0,"scanning":false}})
}
#[test]
fn scoped_inputs_work_limits_and_errors_are_checked_without_partial_results() {
    let s = state();
    for (kind, q) in cases() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            assert_eq!(
                run(call(kind, &f, &s, ctx(&s), &server, &q, &json!({}))),
                Err(ExchangeError::Malformed)
            );
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
                run(call(kind, &f, &s, c, "s", &q, &json!({}))),
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
                run(call(kind, &f, &s, ctx(&s), "s", &q, &json!({}))),
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
        let e = run(call(kind, &f, &s, ctx(&s), "s", &q, &json!({}))).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    let f = fake(vec![reply(
        json!({"internetRadioStations":{"internetRadioStation":[{"id":"r","name":"R","streamUrl":"inert"},{"id":"r","name":"R","streamUrl":"inert"}]}}),
    )]);
    let mut c = ctx(&s);
    c.limits.items = 2;
    assert_eq!(
        run(call(
            "getInternetRadioStations",
            &f,
            &s,
            c,
            "other",
            &json!({}),
            &json!({})
        )),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![]);
    assert_eq!(
        run(call(
            "getTopSongs",
            &f,
            &s,
            ctx(&s),
            "s",
            &json!({"artistId":"a","limit":9007199254740992u64}),
            &json!({})
        )),
        Err(ExchangeError::Malformed)
    );
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn pending_and_final_completion_guards_cover_direct_and_composed_reads() {
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
    // Composed reads perform their own preflight before the selected-song read.
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
    for (kind, q) in cases() {
        for cancel in [false, true] {
            let s = state();
            let features = json!({});
            let mut future = Box::pin(call(kind, &Pending, &s, ctx(&s), "s", &q, &features));
            let w = Waker::from(Arc::new(Noop));
            let mut cx = Context::from_waker(&w);
            assert!(future.as_mut().poll(&mut cx).is_pending());
            if cancel {
                s.cancelled.set(true);
            } else {
                s.now.set(11);
            }
            let error = ExchangeError::Context(if cancel {
                DomainError::Cancelled
            } else {
                DomainError::Deadline
            });
            assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(Err(error)));
            drop(future);
            for malformed in [false, true] {
                let s = state();
                let composed = kind == "getFavoriteSongs" || q["type"] == "personal";
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                    after: if composed { 7 } else { 5 },
                };
                let f = fake(vec![reply(if malformed {
                    json!({"artist":3,"topSongs":3,"internetRadioStations":3,"scanStatus":3})
                } else {
                    empty_body()
                })]);
                assert_eq!(
                    run(call(kind, &f, &clock, ctx(&s), "s", &q, &json!({}))),
                    Err(error),
                    "{kind}"
                );
            }
        }
    }
}
