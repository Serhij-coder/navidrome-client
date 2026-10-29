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
        "../fixtures/p03/subsonic-collection-reads.json"
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

fn folders(q: &Value) -> Option<Vec<FolderId>> {
    q.get("musicFolderId").filter(|v| !v.is_null()).map(|v| {
        if let Some(s) = v.as_str() {
            vec![FolderId::new(s)]
        } else {
            v.as_array()
                .unwrap()
                .iter()
                .map(|v| FolderId::new(v.as_str().unwrap()))
                .collect()
        }
    })
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
    match op {
        "getMusicFolderList" => get_music_folders(f,clock,c,&base,&auth,&server).await.map(|p|{
            assert!(p.items.iter().all(|i|i.server==server));
            json!({"items":p.items.iter().map(|i|json!({"id":i.id.as_str(),"name":i.name})).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})
        }),
        "getRoles" => get_artist_roles(f,clock,c,&base,&auth,&server).await.map(|items|json!(items.iter().map(|i|match i {ArtistRoleChoice::AllArtists=>json!({"label":"all artists","value":""}),ArtistRoleChoice::Role(s)=>json!(s)}).collect::<Vec<_>>())),
        "getRandomSongList" => {
            let q=RandomSongQuery{genre:q["genre"].as_str().map(str::to_owned),limit:q["limit"].as_u64(),max_year:q["maxYear"].as_i64().map(|v|v.try_into().unwrap()),min_year:q["minYear"].as_i64().map(|v|v.try_into().unwrap()),music_folder_ids:folders(q),played:match q["played"].as_str(){Some("never")=>Played::Never,Some("played")=>Played::Played,_=>Played::All}};
            get_random_songs(f,clock,c,&base,&auth,&server,&q).await.map(|p| {
                assert!(p.items.iter().all(|i|i.server==server && i.genres.iter().all(|g|g.server==server)));
                json!({"items":p.items.iter().map(projections::song).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})
            })
        },
        "getSimilarSongs" => {
            let i=Scoped{server:server.clone(),id:SongId::new(q["songId"].as_str().unwrap_or("same"))};
            let q=SimilarSongsQuery{count:q["count"].as_u64(),music_folder_ids:folders(q)};
            get_similar_songs(f,clock,c,&base,&auth,&i,&q).await.map(|songs|{
                assert!(songs.iter().all(|s|s.server==server));
                json!(songs.iter().map(projections::song).collect::<Vec<_>>())
            })
        },
        _=>panic!("fixture operation"),
    }
}
fn project(value: &mut Value) {
    if value["_itemType"] == "song" {
        value["duration"] = json!(value["duration"].as_f64().unwrap() as u64);
        value["playlistItemId"] = Value::Null;
        for key in ["gain", "peak"] {
            if value[key].is_object() {
                for side in ["album", "track"] {
                    if value[key].get(side).is_none() {
                        value[key][side] = Value::Null;
                    }
                }
            }
        }
    }
    if let Some(a) = value.as_array_mut() {
        for v in a {
            project(v);
        }
    }
    if let Some(a) = value.get_mut("items").and_then(Value::as_array_mut) {
        for v in a {
            project(v);
        }
    }
}
#[test]
fn pinned_controllers_paths_parameters_and_normalized_fields() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let f = fake(case["input"].clone());
        let result = poll(call(
            case["operation"].as_str().unwrap(),
            &f,
            &s,
            ctx(&s),
            "server-a",
            &case["query"],
        ));
        if case["target_error"] == true || case.get("expected_error").is_some() {
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
        let actual: Vec<_> = url
            .query_pairs()
            .filter(|(k, _)| !["c", "f", "v", "u", "p"].contains(&k.as_ref()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let expected: std::collections::BTreeMap<_, _> = case["request"]["args"]["query"]
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
        assert_eq!(
            actual
                .iter()
                .cloned()
                .collect::<std::collections::BTreeMap<_, _>>(),
            expected
        );
        assert_eq!(actual.len(), expected.len());
    }
}
#[test]
fn work_bounds_strict_raw_ids_and_utf16_sort() {
    let fixture = fixture();
    let s = state();
    let roles = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "roles-rich")
        .unwrap();
    let f = fake(roles["input"].clone());
    let value = poll(call("getRoles", &f, &s, ctx(&s), "s", &json!({}))).unwrap();
    let a = value.as_array().unwrap();
    assert_eq!(a.last().unwrap(), "\u{e000}");
    assert_eq!(a[a.len() - 2], "😀");
    let input = json!({"artists":{"index":[{"artist":[{"roles":["same","same","same"]}]}]}});
    let f = fake(input);
    let mut c = ctx(&s);
    c.limits.items = 5;
    assert_eq!(
        poll(call("getRoles", &f, &s, c, "s", &json!({}))),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 6;
    assert!(poll(call("getRoles", &f, &s, c, "s", &json!({}))).is_ok());
    let f = fake(json!({"similarSongs":{"song":[{"id":"same"},{"id":"same"}]}}));
    c.limits.items = 2;
    assert_eq!(
        poll(call(
            "getSimilarSongs",
            &f,
            &s,
            c,
            "s",
            &json!({"songId":"same"})
        )),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 3;
    assert_eq!(
        poll(call(
            "getSimilarSongs",
            &f,
            &s,
            c,
            "s",
            &json!({"songId":"same"})
        ))
        .unwrap(),
        json!([])
    );
    let f =
        fake(json!({"musicFolders":{"musicFolder":[{"id":"f","name":"F"},{"id":"f","name":"F"}]}}));
    c.limits.items = 2;
    assert_eq!(
        poll(call("getMusicFolderList", &f, &s, c, "s", &json!({}))),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 3;
    assert!(poll(call("getMusicFolderList", &f, &s, c, "s", &json!({}))).is_ok());
}
#[test]
fn invalid_input_limits_and_correlated_wire_errors() {
    let s = state();
    for op in [
        "getMusicFolderList",
        "getRoles",
        "getRandomSongList",
        "getSimilarSongs",
    ] {
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
            Err(ExchangeError::Http(HttpError::Authentication))
        );
        f.status = 200;
        f.body=br#"{"subsonic-response":{"status":"failed","version":"1","error":{"code":50,"message":"private-marker"}}}"#.to_vec();
        let e = poll(call(op, &f, &s, ctx(&s), "s", &json!({}))).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Forbidden));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    for (op, q) in [
        ("getRandomSongList", json!({"limit":9007199254740992u64})),
        ("getSimilarSongs", json!({"count":9007199254740992u64})),
        ("getSimilarSongs", json!({"songId":""})),
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
        calls: Cell<u64>,
        s: &'a State,
        cancel: bool,
    }
    impl Clock for Late<'_> {
        fn now(&self) -> Milliseconds {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n > 5 {
                if self.cancel {
                    self.s.cancelled.set(true);
                }
                Milliseconds(11)
            } else {
                Milliseconds(10)
            }
        }
    }
    for op in [
        "getMusicFolderList",
        "getRoles",
        "getRandomSongList",
        "getSimilarSongs",
    ] {
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
                    calls: Cell::new(0),
                    s: &s,
                    cancel,
                };
                let body = if malformed {
                    json!({"musicFolders":3,"artists":3,"randomSongs":3,"similarSongs":3})
                } else {
                    json!({"musicFolders":{"musicFolder":[]}})
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
}
