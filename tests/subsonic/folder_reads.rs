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
    if v["_itemType"] == "folder" && v.get("parentId").is_none() {
        v["parentId"] = Value::Null;
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
fn query(v: &Value) -> FolderQuery {
    FolderQuery {
        identity: Scoped {
            server: ServerId::new("server-a"),
            id: FolderId::new(v["id"].as_str().unwrap_or("dir")),
        },
        sort_by: SongListSort::ALL
            .iter()
            .copied()
            .find(|s| Some(s.as_str()) == v["sortBy"].as_str())
            .unwrap_or(SongListSort::Name),
        sort_order: if v["sortOrder"] == "DESC" {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        },
        music_folder_ids: v.get("musicFolderId").map(|v| {
            if let Some(s) = v.as_str() {
                vec![FolderId::new(s)]
            } else {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| FolderId::new(v.as_str().unwrap()))
                    .collect()
            }
        }),
        search_term: v["searchTerm"].as_str().map(str::to_owned),
    }
}
fn read(
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    q: &FolderQuery,
    r: &mut Rng,
) -> Result<Folder, ExchangeError> {
    run(get_folder(
        f,
        clock,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        q,
        r,
    ))
}
fn project(f: &Folder) -> Value {
    let children = f
        .children
        .as_ref()
        .expect("source folder always has children");
    let mut v = json!({"_itemType":"folder","_serverId":f.identity.server.as_str(),"_serverType":"subsonic","id":f.identity.id.as_str(),"name":f.name,"parentId":f.parent_id.as_ref().map(FolderId::as_str),"children":{"folders":children.folders.iter().map(project).collect::<Vec<_>>(),"songs":children.songs.iter().map(projections::song).collect::<Vec<_>>()}});
    assert_eq!(f.server_type, ServerType::Subsonic);
    for (key, field) in [("imageId", &f.image_id), ("imageUrl", &f.image_url)] {
        match field {
            Field::Absent => {}
            Field::Null => v[key] = Value::Null,
            Field::Value(s) => v[key] = json!(s),
        }
    }
    v
}
#[test]
fn original_root_directory_sorts_filtering_and_nested_fields() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/p03/subsonic-folder-reads.json")).unwrap();
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
        r.draws = fixture["random_draws"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse::<u64>().unwrap())
            .collect();
        let result = read(&f, &s, ctx(&s), &query(&case["query"]), &mut r);
        match case["target_error"].as_str() {
            Some("Malformed") => {
                assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"])
            }
            Some("Stale") => assert_eq!(result, Err(ExchangeError::Stale)),
            Some("Http") => assert!(matches!(result, Err(ExchangeError::Http(_)))),
            Some("Protocol") => assert_eq!(
                result,
                Err(ExchangeError::Protocol(ProtocolError::Authentication))
            ),
            None => {
                let actual = project(&result.unwrap_or_else(|e| panic!("{}: {e:?}", case["name"])));
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
        assert_eq!(urls.len(), requests.len());
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
fn song() -> Value {
    json!({"id":"same","title":"Song","created":"2020-01-01T00:00:00Z","size":1,"contentType":"audio/flac"})
}
#[test]
fn complete_tree_and_raw_indexes_are_bounded_before_filtering_and_keep_scope() {
    let s = state();
    let mut q = query(&json!({"id":"0","searchTerm":"absent"}));
    let body = json!({"indexes":{"index":[{"artist":[{"id":"a","name":"A"},{"id":"b","name":"B"},{"id":"c","name":"C"}]}]}});
    let f = fake(vec![reply(body.clone())]);
    let mut c = ctx(&s);
    c.limits.items = 4;
    assert_eq!(
        read(&f, &s, c, &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![reply(body)]);
    c.limits.items = 5;
    assert!(
        read(&f, &s, c, &q, &mut rng())
            .unwrap()
            .children
            .unwrap()
            .folders
            .is_empty()
    );
    q = query(&json!({"searchTerm":"absent"}));
    let body = json!({"directory":{"id":"dir","title":"D","child":[{"id":"f","title":"F","isDir":true,"child":[song(),song()]},song()]}});
    let f = fake(vec![reply(body.clone())]);
    c.limits.items = 4;
    assert_eq!(
        read(&f, &s, c, &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![reply(body)]);
    c.limits.items = 5;
    assert!(
        read(&f, &s, c, &q, &mut rng())
            .unwrap()
            .children
            .unwrap()
            .folders
            .is_empty()
    );
    q = query(&json!({}));
    q.identity.server = ServerId::new("other");
    let f = fake(vec![reply(
        json!({"directory":{"id":"dir","title":"D","child":[{"id":"f","title":"F","isDir":true,"child":[song()]},song(),song()]}}),
    )]);
    let folder = read(&f, &s, ctx(&s), &q, &mut rng()).unwrap();
    let children = folder.children.unwrap();
    assert_eq!(children.folders[0].identity.server, q.identity.server);
    assert_eq!(
        children.folders[0].children.as_ref().unwrap().songs[0].server,
        q.identity.server
    );
    assert_eq!(children.songs.len(), 2);
    assert!(
        children
            .songs
            .iter()
            .all(|s| s.server == q.identity.server && s.playlist_item_id.is_none())
    );
}
#[test]
fn input_and_typed_wire_errors_never_publish_partial_folders() {
    let s = state();
    for root in [false, true] {
        let q = query(&json!({"id":if root{"0"}else{"dir"}}));
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            let mut q = q.clone();
            q.identity.server = ServerId::new(server);
            assert_eq!(
                read(&f, &s, ctx(&s), &q, &mut rng()),
                Err(ExchangeError::Malformed)
            );
            assert!(f.urls.borrow().is_empty());
        }
        for request in [true, false] {
            let f = fake(vec![reply(json!({}))]);
            let mut c = ctx(&s);
            if request {
                c.limits.request_bytes = 1;
            } else {
                c.limits.response_bytes = 1;
            }
            assert_eq!(
                read(&f, &s, c, &q, &mut rng()),
                Err(ExchangeError::Context(DomainError::Capacity))
            );
            assert_eq!(f.urls.borrow().len(), usize::from(!request));
        }
        let f = fake(vec![Reply {
            stale: true,
            ..reply(json!({}))
        }]);
        assert_eq!(
            read(&f, &s, ctx(&s), &q, &mut rng()),
            Err(ExchangeError::Stale)
        );
        let f = fake(vec![Reply {
            error: Some(TransportError::Network),
            ..reply(json!({}))
        }]);
        assert_eq!(
            read(&f, &s, ctx(&s), &q, &mut rng()),
            Err(ExchangeError::Transport(TransportError::Network))
        );
        let f = fake(vec![reply(
            json!({"status":"failed","error":{"code":40,"message":"private-marker"}}),
        )]);
        let e = read(&f, &s, ctx(&s), &q, &mut rng()).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
    let f = fake(vec![]);
    let mut q = query(&json!({"id":""}));
    assert_eq!(
        read(&f, &s, ctx(&s), &q, &mut rng()),
        Err(ExchangeError::Malformed)
    );
    q = query(&json!({"searchTerm":"x".repeat(8193)}));
    assert_eq!(
        read(&f, &s, ctx(&s), &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    q = query(&json!({"musicFolderId":["a","b"]}));
    let mut c = ctx(&s);
    c.limits.items = 1;
    assert_eq!(
        read(&f, &s, c, &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn pending_and_final_guards_cover_root_directory_and_nested_errors() {
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
    for root in [true, false] {
        for cancel in [false, true] {
            let s = state();
            let q = query(&json!({"id":if root{"0"}else{"dir"}}));
            let base = ServerBase::parse("http://fixture.invalid").unwrap();
            let auth = Auth::legacy("dummy", "dummy").unwrap();
            let mut r = rng();
            let mut future = Box::pin(get_folder(&Pending, &s, ctx(&s), &base, &auth, &q, &mut r));
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
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                };
                let f = fake(vec![reply(if malformed {
                    json!({"indexes":3,"directory":{"id":"dir","title":"D","child":[null]}})
                } else {
                    json!({"indexes":{"index":[]},"directory":{"id":"dir","title":"D"}})
                })]);
                assert_eq!(read(&f, &clock, ctx(&s), &q, &mut rng()), Err(error));
            }
        }
    }
}
