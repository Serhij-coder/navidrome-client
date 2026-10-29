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
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-playlist-reads.json")).unwrap()
}
fn fake(kind: &str, input: Value) -> Fake {
    Fake {
        body: serde_json::to_vec(
            &json!({"subsonic-response":{"status":"ok","version":"1.16.1",kind:input}}),
        )
        .unwrap(),
        status: 200,
        stale: false,
        error: None,
        urls: RefCell::new(vec![]),
    }
}

fn base() -> ServerBase {
    ServerBase::parse("http://fixture.invalid/music").unwrap()
}
fn auth() -> Auth {
    Auth::legacy("dummy", "dummy").unwrap()
}
fn identity(server: &str, id: &str) -> Scoped<PlaylistId> {
    Scoped {
        server: ServerId::new(server),
        id: PlaylistId::new(id),
    }
}
fn metadata(p: &Playlist) -> Value {
    assert_eq!(p.server_type, ServerType::Subsonic);
    assert!(p.genres.is_empty());
    assert_eq!(p.rules, Field::Absent);
    assert_eq!(p.sync, Field::Absent);
    assert_eq!(p.uploaded_image, None);
    json!({"_itemType":"playlist","_serverId":p.server.as_str(),"_serverType":"subsonic","blurHash":p.blur_hash,"description":p.description,"dominantColor":p.dominant_color,"duration":p.duration.map(|v|v.0),"evaluatedAt":p.evaluated_at,"genres":[],"id":p.id.as_str(),"imageId":p.image_id,"imageUrl":p.image_url,"name":p.name,"owner":p.owner,"ownerId":p.owner_id.as_ref().map(UserId::as_str),"public":p.public,"size":p.size,"songCount":p.song_count,"thumbHash":p.thumb_hash})
}
fn output(
    operation: &str,
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    id: &str,
) -> Result<Value, ExchangeError> {
    let b = base();
    let a = auth();
    let i = identity(server, id);
    match operation {
        "getPlaylistDetail" => poll(get_playlist(f,clock,c,&b,&a,&i)).map(|p|metadata(&p)),
        "getPlaylistSongList" => poll(get_playlist_songs(f,clock,c,&b,&a,&i)).map(|p| {
            for entry in &p.items { assert_eq!(entry.song.playlist_item_id.as_ref(),Some(&entry.entry)); }
            json!({"items":p.items.iter().map(|e|projections::song(&e.song)).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})
        }),
        "getPlaylistSongIds" => poll(get_playlist_song_ids(f,clock,c,&b,&a,&i)).map(|p|json!({"items":p.items.iter().map(SongId::as_str).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})),
        _ => panic!("fixture operation"),
    }
}
fn project_expected(value: &mut Value) {
    if value["_itemType"] == "playlist" {
        for key in ["owner", "ownerId", "public", "songCount"] {
            if value.get(key).is_none() {
                value[key] = Value::Null;
            }
        }
        if let Some(duration) = value["duration"].as_f64() {
            value["duration"] = json!(duration as u64);
        }
    }
    if let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) {
        for song in items {
            if song.is_object() {
                song["duration"] = json!(song["duration"].as_f64().unwrap() as u64);
                for key in ["gain", "peak"] {
                    if song[key].is_object() {
                        for side in ["album", "track"] {
                            if song[key].get(side).is_none() {
                                song[key][side] = Value::Null;
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn pinned_controllers_and_every_normalized_field() {
    for case in fixture()["cases"].as_array().unwrap() {
        let s = state();
        let f = fake("playlist", case["input"].clone());
        let result = output(
            case["operation"].as_str().unwrap(),
            &f,
            &s,
            ctx(&s),
            "server-a",
            case["request_id"].as_str().unwrap(),
        );
        if case["target_error"] == true || case.get("expected_error").is_some() {
            assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"]);
        } else {
            let mut expected = case["expected"].clone();
            project_expected(&mut expected);
            projections::equivalent(&result.unwrap(), &expected);
        }
        let urls = f.urls.borrow();
        let parsed = url::Url::parse(&urls[0]).unwrap();
        assert_eq!(parsed.path(), "/music/rest/getPlaylist.view");
        assert_eq!(
            parsed
                .query_pairs()
                .filter(|(k, _)| k == "id")
                .map(|(_, v)| v.into_owned())
                .collect::<Vec<_>>(),
            [case["request_id"].as_str().unwrap()]
        );
    }
}
#[test]
fn entries_retain_order_occurrence_scope_and_aggregate_bounds() {
    let cases = fixture();
    let input = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "repeated-songs")
        .unwrap()["input"]
        .clone();
    let s = state();
    let f = fake("playlist", input.clone());
    let page = poll(get_playlist_songs(
        &f,
        &s,
        ctx(&s),
        &base(),
        &auth(),
        &identity("other-server", "playlist-1"),
    ))
    .unwrap();
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.total_record_count, Some(3));
    assert_eq!(page.start_index, 0);
    assert_eq!(page.items[0].song.id, page.items[1].song.id);
    assert_ne!(page.items[0].entry, page.items[1].entry);
    for (index, item) in page.items.iter().enumerate() {
        assert_eq!(item.entry.as_str(), index.to_string());
        assert_eq!(item.song.server.as_str(), "other-server");
        assert!(
            item.song
                .genres
                .iter()
                .all(|g| g.server == item.song.server)
        );
        assert_eq!(item.song.disc_subtitle, None);
    }
    let bare = json!({"id":"same","title":"Synthetic","contentType":"audio/flac","created":"raw","size":1});
    let f = fake(
        "playlist",
        json!({"id":"playlist-1","entry":[bare.clone(),bare.clone()]}),
    );
    let mut c = ctx(&s);
    c.limits.items = 2;
    assert_eq!(
        output("getPlaylistSongList", &f, &s, c, "s", "playlist-1"),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 3;
    assert!(output("getPlaylistSongList", &f, &s, c, "s", "playlist-1").is_ok());
    let mut nested = bare;
    nested["artists"] = json!([{"id":"a","name":"A"}]);
    let f = fake("playlist", json!({"id":"playlist-1","entry":[nested]}));
    c.limits.items = 2;
    assert_eq!(
        output("getPlaylistSongList", &f, &s, c, "s", "playlist-1"),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 3;
    assert!(output("getPlaylistSongList", &f, &s, c, "s", "playlist-1").is_ok());
}
#[test]
fn invalid_identity_protocol_limits_and_completion_correlation() {
    let s = state();
    let f = fake(
        "playlist",
        json!({"id":"playlist-1","name":"P","duration":0}),
    );
    for op in [
        "getPlaylistDetail",
        "getPlaylistSongList",
        "getPlaylistSongIds",
    ] {
        let before = f.urls.borrow().len();
        for server in ["".into(), " ".into(), "x".repeat(129)] {
            assert_eq!(
                output(op, &f, &s, ctx(&s), &server, "playlist-1"),
                Err(ExchangeError::Malformed)
            );
        }
        assert_eq!(
            output(op, &f, &s, ctx(&s), "s", ""),
            Err(ExchangeError::Malformed)
        );
        let mut c = ctx(&s);
        c.limits.items = 0;
        assert_eq!(
            output(op, &f, &s, c, "s", "playlist-1"),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        assert_eq!(f.urls.borrow().len(), before);
        assert_eq!(
            output(op, &f, &s, ctx(&s), "s", "wrong"),
            Err(ExchangeError::Stale)
        );
        for request in [true, false] {
            let mut c = ctx(&s);
            if request {
                c.limits.request_bytes = 1;
            } else {
                c.limits.response_bytes = 1;
            }
            assert_eq!(
                output(op, &f, &s, c, "s", "playlist-1"),
                Err(ExchangeError::Context(DomainError::Capacity))
            );
        }
        let mut bad = fake("playlist", json!({"id":"playlist-1"}));
        bad.stale = true;
        assert_eq!(
            output(op, &bad, &s, ctx(&s), "s", "playlist-1"),
            Err(ExchangeError::Stale)
        );
        bad.stale = false;
        bad.error = Some(TransportError::Network);
        assert_eq!(
            output(op, &bad, &s, ctx(&s), "s", "playlist-1"),
            Err(ExchangeError::Transport(TransportError::Network))
        );
        bad.error = None;
        bad.status = 503;
        assert!(matches!(
            output(op, &bad, &s, ctx(&s), "s", "playlist-1"),
            Err(ExchangeError::Http(_))
        ));
        bad.status = 200;
        bad.body=br#"{"subsonic-response":{"status":"failed","version":"1","error":{"code":40,"message":"private-marker"}}}"#.to_vec();
        let e = output(op, &bad, &s, ctx(&s), "s", "playlist-1").unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
    }
}

fn invoke<'a>(
    op: &'a str,
    f: &'a dyn ProtocolTransport,
    clock: &'a dyn Clock,
    c: OperationContext<'a>,
    b: &'a ServerBase,
    a: &'a Auth,
    i: &'a Scoped<PlaylistId>,
) -> LocalFuture<'a, Result<(), ExchangeError>> {
    Box::pin(async move {
        match op {
            "getPlaylistDetail" => get_playlist(f, clock, c, b, a, i).await.map(|_| ()),
            "getPlaylistSongList" => get_playlist_songs(f, clock, c, b, a, i).await.map(|_| ()),
            "getPlaylistSongIds" => get_playlist_song_ids(f, clock, c, b, a, i)
                .await
                .map(|_| ()),
            _ => panic!("fixture operation"),
        }
    })
}
#[test]
fn pending_and_final_context_guards_cover_all_three_reads() {
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
        after: u64,
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
    for op in [
        "getPlaylistDetail",
        "getPlaylistSongList",
        "getPlaylistSongIds",
    ] {
        let b = base();
        let a = auth();
        let i = identity("s", "playlist-1");
        for cancel in [true, false] {
            let s = state();
            let mut future = invoke(op, &Pending, &s, ctx(&s), &b, &a, &i);
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
            for mode in 0..4 {
                let s = state();
                let clock = Late {
                    calls: Cell::new(0),
                    s: &s,
                    cancel,
                    after: 5,
                };
                let mut input = json!({"id":"playlist-1","name":"P","duration":0});
                match mode {
                    0 => input = Value::Null,
                    1 => {
                        input["name"] = Value::Null;
                        input["entry"] = json!([null]);
                    }
                    2 => input["id"] = json!("wrong"),
                    _ => (),
                }
                let f = fake("playlist", input);
                assert_eq!(
                    poll(invoke(op, &f, &clock, ctx(&s), &b, &a, &i)),
                    Err(ExchangeError::Context(if cancel {
                        DomainError::Cancelled
                    } else {
                        DomainError::Deadline
                    })),
                    "{op} mode {mode}"
                );
            }
        }
    }
    let s = state();
    let clock = Late {
        calls: Cell::new(0),
        s: &s,
        cancel: false,
        after: 6,
    };
    let f = fake("playlist", json!({"id":"playlist-1"}));
    assert_eq!(
        output("getPlaylistSongIds", &f, &clock, ctx(&s), "s", "playlist-1"),
        Err(ExchangeError::Context(DomainError::Deadline))
    );
}
