#![recursion_limit = "256"]
use feishin_domain::*;
use feishin_subsonic::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
#[path = "support/projection.rs"]
mod projections;
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
            request: RequestId(1),
            generation: Generation(2),
            deadline: Milliseconds(10),
        },
        cancellation: s,
        limits: OperationLimits {
            request_bytes: 10000,
            response_bytes: 1000000,
            items: 1000,
        },
    }
}
struct Fake {
    body: Vec<u8>,
    status: u16,
    stale: bool,
    error: Option<TransportError>,
    urls: RefCell<Vec<String>>,
}
impl ProtocolTransport for Fake {
    fn exchange<'a>(
        &'a self,
        req: PlannedRequest,
        context: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
        self.urls
            .borrow_mut()
            .push(req.expose_url_for_transport().into());
        assert!(matches!(req.method(), Method::Get));
        Box::pin(async move {
            if let Some(e) = self.error {
                return Err(e);
            }
            let mut correlation = context.correlation;
            if self.stale {
                correlation.request = RequestId(9)
            }
            Ok(TransportResponse {
                correlation,
                status: self.status,
                body: self.body.clone(),
            })
        })
    }
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
fn poll<F: Future>(future: F) -> F::Output {
    let w = Waker::from(Arc::new(Noop));
    let mut c = Context::from_waker(&w);
    match std::pin::pin!(future).as_mut().poll(&mut c) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("ready fixture"),
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../fixtures/p03/subsonic-enrichment-reads.json"
    ))
    .unwrap()
}
fn fake(input: Value) -> Fake {
    let mut body = input.as_object().unwrap().clone();
    body.insert("status".into(), json!("ok"));
    body.insert("version".into(), json!("1.16.1"));
    Fake {
        body: serde_json::to_vec(&json!({"subsonic-response":body})).unwrap(),
        status: 200,
        stale: false,
        error: None,
        urls: RefCell::new(vec![]),
    }
}

async fn call(
    op: &str,
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    q: &Value,
) -> Result<Value, ExchangeError> {
    let base = ServerBase::parse("http://fixture.invalid/music").unwrap();
    let auth = Auth::legacy("dummy", "dummy").unwrap();
    let server = ServerId::new(server);
    let limit = q["count"].as_u64().or(q["limit"].as_u64());
    match op{
        "getAlbumRadio"=>get_album_radio(f,clock,c,&base,&auth,&Scoped{server:server.clone(),id:AlbumId::new(q["albumId"].as_str().unwrap_or("same"))},limit).await.map(|songs|{
            assert!(songs.iter().all(|s|s.server==server && s.server_type==ServerType::Subsonic));json!(songs.iter().map(projections::song).collect::<Vec<_>>())
        }),
        "getArtistRadio"=>get_artist_radio(f,clock,c,&base,&auth,&Scoped{server:server.clone(),id:ArtistId::new(q["artistId"].as_str().unwrap_or("same"))},limit).await.map(|songs|{
            assert!(songs.iter().all(|s|s.server==server && s.server_type==ServerType::Subsonic));json!(songs.iter().map(projections::song).collect::<Vec<_>>())
        }),
        "getAlbumArtistInfo"=>get_album_artist_info(f,clock,c,&base,&auth,&Scoped{server,id:ArtistId::new(q["id"].as_str().unwrap_or("artist-1"))},limit).await.map(|info|{
            let Some(info)=info else{return Value::Null;};assert_eq!(info.image_url,Field::Absent);
            let biography=match info.biography{Field::Null=>Value::Null,Field::Value(s)=>json!(s),Field::Absent=>panic!("biography must be explicit")};
            json!({"biography":biography,"similarArtists":info.similar_artists.as_ref().map(|v|v.iter().map(projections::related).collect::<Vec<_>>())})
        }),
        _=>panic!("fixture operation"),
    }
}
fn project(v: &mut Value) {
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
    if let Some(a) = v.as_array_mut() {
        for v in a {
            project(v);
        }
    }
    if let Some(a) = v.get_mut("similarArtists").and_then(Value::as_array_mut) {
        for v in a {
            if v["imageId"].is_number() {
                v["imageId"] = json!(v["imageId"].to_string());
            }
        }
    }
}
#[test]
fn original_controllers_enrichment_fields_radio_order_and_paths() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let mut f = fake(case["input"].clone());
        f.status = case["status"].as_u64().unwrap_or(200).try_into().unwrap();
        let op = case["operation"].as_str().unwrap();
        let result = poll(call(op, &f, &s, ctx(&s), "server-a", &case["query"]));
        if case["expected_error"] == "Error" {
            assert_eq!(result, Err(ExchangeError::Http(HttpError::Server)));
        } else if case["target_error"] == true || case.get("expected_error").is_some() {
            assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"]);
        } else {
            let mut expected = case["expected"].clone();
            project(&mut expected);
            projections::equivalent(&result.unwrap(), &expected);
        }
        let urls = f.urls.borrow();
        let url = url::Url::parse(&urls[0]).unwrap();
        let endpoint = case["request"]["endpoint"].as_str().unwrap();
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
        let expected: std::collections::BTreeMap<_, _> = case["request"]["args"]["query"]
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
        assert_eq!(actual, expected);
    }
}
#[test]
fn budgets_duplicates_optional_lists_and_http_fallback() {
    let s = state();
    let f = fake(
        json!({"artistInfo2":{"similarArtist":[{"id":"a","name":"A","coverArt":"","userRating":0},{"id":"a","name":"A"}]}}),
    );
    let mut c = ctx(&s);
    c.limits.items = 2;
    assert_eq!(
        poll(call("getAlbumArtistInfo", &f, &s, c, "s", &json!({}))),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 3;
    let info = poll(call("getAlbumArtistInfo", &f, &s, c, "s", &json!({}))).unwrap();
    assert_eq!(info["similarArtists"][0]["imageId"], "");
    assert_eq!(info["similarArtists"][0]["userRating"].as_f64(), Some(0.0));
    assert_eq!(info["similarArtists"][1]["imageId"], "a");
    for body in [json!({}), json!({"artistInfo2":null})] {
        let f = fake(body);
        assert_eq!(
            poll(call("getAlbumArtistInfo", &f, &s, ctx(&s), "s", &json!({}))).unwrap(),
            json!({"biography":null,"similarArtists":null})
        );
    }
    for status in [201, 400, 401, 403, 404, 500] {
        let mut f = fake(json!({}));
        f.status = status;
        assert_eq!(
            poll(call("getAlbumArtistInfo", &f, &s, ctx(&s), "s", &json!({}))).unwrap(),
            Value::Null
        );
    }
    let song = json!({"id":"same","title":"S","contentType":"audio/flac","created":"raw","size":0});
    for (op, parent) in [
        ("getAlbumRadio", "similarSongs"),
        ("getArtistRadio", "similarSongs2"),
    ] {
        let f = fake(json!({parent:{"song":[song.clone(),song.clone()]}}));
        c.limits.items = 2;
        assert_eq!(
            poll(call(op, &f, &s, c, "s", &json!({}))),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        c.limits.items = 3;
        let result = poll(call(op, &f, &s, c, "s", &json!({}))).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 2);
        assert_eq!(result[0]["id"], result[1]["id"]);
    }
}

#[test]
fn invalid_input_limits_and_correlated_wire_errors() {
    let s = state();
    for op in ["getAlbumRadio", "getArtistRadio", "getAlbumArtistInfo"] {
        let f = fake(json!({}));
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            assert_eq!(
                poll(call(op, &f, &s, ctx(&s), &server, &json!({}))),
                Err(ExchangeError::Malformed)
            );
        }
        let mut c = ctx(&s);
        c.limits.items = 0;
        assert_eq!(
            poll(call(op, &f, &s, c, "s", &json!({}))),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        assert!(f.urls.borrow().is_empty());
        for request in [true, false] {
            let mut c = ctx(&s);
            if request {
                c.limits.request_bytes = 1;
            } else {
                c.limits.response_bytes = 1;
            }
            assert_eq!(
                poll(call(op, &f, &s, c, "s", &json!({}))),
                Err(ExchangeError::Context(DomainError::Capacity))
            );
        }
        let mut f = fake(json!({}));
        f.stale = true;
        assert_eq!(
            poll(call(op, &f, &s, ctx(&s), "s", &json!({}))),
            Err(ExchangeError::Stale)
        );
        f.stale = false;
        f.error = Some(TransportError::Network);
        assert_eq!(
            poll(call(op, &f, &s, ctx(&s), "s", &json!({}))),
            Err(ExchangeError::Transport(TransportError::Network))
        );
        f.error = None;
        f.status = 401;
        assert_eq!(
            poll(call(op, &f, &s, ctx(&s), "s", &json!({}))),
            if op == "getAlbumArtistInfo" {
                Ok(Value::Null)
            } else {
                Err(ExchangeError::Http(HttpError::Authentication))
            }
        );
        f.status = 200;
        f.body=br#"{"subsonic-response":{"status":"failed","version":"1","error":{"code":50,"message":"private-marker"}}}"#.to_vec();
        let e = poll(call(op, &f, &s, ctx(&s), "s", &json!({}))).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Forbidden));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    for (op, q) in [
        ("getAlbumRadio", json!({"count":9007199254740992u64})),
        ("getArtistRadio", json!({"count":9007199254740992u64})),
        ("getAlbumArtistInfo", json!({"limit":9007199254740992u64})),
        ("getAlbumArtistInfo", json!({"id":""})),
    ] {
        let f = fake(json!({}));
        assert_eq!(
            poll(call(op, &f, &s, ctx(&s), "s", &q)),
            Err(ExchangeError::Malformed)
        );
        assert!(f.urls.borrow().is_empty());
    }
}
#[test]
fn pending_and_post_normalization_context_priority() {
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
        after: u64,
        calls: Cell<u64>,
        s: &'a State,
        cancel: bool,
    }
    impl Clock for Late<'_> {
        fn now(&self) -> Milliseconds {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n > self.after {
                if self.cancel {
                    self.s.cancelled.set(true);
                }
                Milliseconds(11)
            } else {
                Milliseconds(10)
            }
        }
    }
    for op in ["getAlbumRadio", "getArtistRadio", "getAlbumArtistInfo"] {
        for cancel in [false, true] {
            let s = state();
            let q = json!({});
            let mut future = Box::pin(call(op, &Pending, &s, ctx(&s), "s", &q));
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
                    after: 5,
                    calls: Cell::new(0),
                    s: &s,
                    cancel,
                };
                let body = if malformed {
                    json!({"artistInfo2":3,"similarSongs":3,"similarSongs2":3})
                } else {
                    json!({})
                };
                let f = fake(body);
                assert_eq!(
                    poll(call(op, &f, &clock, ctx(&s), "s", &json!({}))),
                    Err(ExchangeError::Context(if cancel {
                        DomainError::Cancelled
                    } else {
                        DomainError::Deadline
                    }))
                );
            }
        }
    }
    for cancel in [false, true] {
        let s = state();
        let clock = Late {
            after: 4,
            calls: Cell::new(0),
            s: &s,
            cancel,
        };
        let mut f = fake(json!({}));
        f.status = 500;
        assert_eq!(
            poll(call(
                "getAlbumArtistInfo",
                &f,
                &clock,
                ctx(&s),
                "s",
                &json!({})
            )),
            Err(ExchangeError::Context(if cancel {
                DomainError::Cancelled
            } else {
                DomainError::Deadline
            }))
        );
    }
}
