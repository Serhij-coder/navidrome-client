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
// Other adapter targets exercise the shared song projector.
#[allow(dead_code)]
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
                total_count_header: None,
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
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-list-reads.json")).unwrap()
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

fn genre(g: &Genre) -> Value {
    assert_eq!(g.server_type, ServerType::Subsonic);
    json!({"_itemType":"genre","_serverId":g.server.as_str(),"_serverType":"subsonic","id":g.id.as_str(),"name":g.name,"albumCount":g.album_count,"songCount":g.song_count,"imageId":g.image_id,"imageUrl":g.image_url})
}
fn metadata(p: &Playlist) -> Value {
    assert_eq!(p.server_type, ServerType::Subsonic);
    assert_eq!(p.rules, Field::Absent);
    assert_eq!(p.sync, Field::Absent);
    assert_eq!(p.uploaded_image, None);
    json!({"_itemType":"playlist","_serverId":p.server.as_str(),"_serverType":"subsonic","blurHash":p.blur_hash,"description":p.description,"dominantColor":p.dominant_color,"duration":p.duration.map(|v|v.0),"evaluatedAt":p.evaluated_at,"genres":p.genres.iter().map(genre).collect::<Vec<_>>(),"id":p.id.as_str(),"imageId":p.image_id,"imageUrl":p.image_url,"name":p.name,"owner":p.owner,"ownerId":p.owner_id.as_ref().map(UserId::as_str),"public":p.public,"size":p.size,"songCount":p.song_count,"thumbHash":p.thumb_hash})
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
    let sort_order = if q["sortOrder"] == "DESC" {
        SortOrder::Desc
    } else {
        SortOrder::Asc
    };
    let start_index = q["startIndex"].as_u64().unwrap_or(0);
    let limit = q["limit"].as_u64();
    let search_term = q["searchTerm"].as_str().map(str::to_owned);
    match op {
        "getPlaylistList" => {
            let sort_by = match q["sortBy"].as_str() {
                Some("duration") => PlaylistListSort::Duration,
                Some("owner") => PlaylistListSort::Owner,
                Some("public") => PlaylistListSort::Public,
                Some("songCount") => PlaylistListSort::SongCount,
                Some("updatedAt") => PlaylistListSort::UpdatedAt,
                _ => PlaylistListSort::Name,
            };
            let query = ListQuery {
                server: server.clone(),
                sort_by,
                sort_order,
                start_index,
                limit,
                search_term,
                filter: PlaylistFilter {
                    exclude_smart_playlists: q["excludeSmartPlaylists"].as_bool(),
                },
            };
            get_playlists(f,clock,c,&base,&auth,&query).await.map(|p|{
                assert!(p.items.iter().all(|i|i.server==server));
                json!({"items":p.items.iter().map(metadata).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})
            })
        }
        "getPlaylistListCount" => {
            get_playlist_count(f, clock, c, &base, &auth, &server, search_term.as_deref())
                .await
                .map(|n| json!(n))
        }
        "getGenreList" => {
            let sort_by = match q["sortBy"].as_str() {
                Some("albumCount") => GenreListSort::AlbumCount,
                Some("songCount") => GenreListSort::SongCount,
                _ => GenreListSort::Name,
            };
            let query = ListQuery {
                server: server.clone(),
                sort_by,
                sort_order,
                start_index,
                limit,
                search_term,
                filter: GenreFilter {
                    music_folder_ids: Some(vec![FolderId::new("ignored")]),
                },
            };
            get_genres(f,clock,c,&base,&auth,&query).await.map(|p|{
                assert!(p.items.iter().all(|i|i.server==server));
                json!({"items":p.items.iter().map(genre).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count})
            })
        }
        _ => panic!("fixture operation"),
    }
}
fn project(v: &mut Value) {
    if let Some(items) = v.get_mut("items").and_then(Value::as_array_mut) {
        for item in items {
            let keys: &[&str] = if item["_itemType"] == "playlist" {
                &["owner", "ownerId", "public", "songCount"]
            } else {
                &["albumCount", "songCount"]
            };
            for key in keys {
                if item.get(*key).is_none() {
                    item[*key] = Value::Null;
                }
            }
            if item["_itemType"] == "playlist" {
                item["duration"] = json!(item["duration"].as_f64().unwrap() as u64);
            }
        }
    }
}
#[test]
fn all_source_sort_search_page_and_count_cases() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let f = fake(case["input"].clone());
        let op = case["operation"].as_str().unwrap();
        let result = poll(call(op, &f, &s, ctx(&s), "server-a", &case["query"]));
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
        assert!(
            url.query_pairs()
                .all(|(k, _)| ["c", "f", "v", "u", "p"].contains(&k.as_ref()))
        );
    }
}
#[test]
fn bounds_apply_before_filtering_and_pagination_and_counts_agree() {
    let s = state();
    let f = fake(
        json!({"playlists":{"playlist":[{"id":"1","name":"One","duration":0},{"id":"2","name":"Two","duration":0}]},"genres":{"genre":[{"value":"A"},{"value":"B"}]}}),
    );
    for op in ["getPlaylistList", "getPlaylistListCount", "getGenreList"] {
        let mut c = ctx(&s);
        c.limits.items = 2;
        assert_eq!(
            poll(call(
                op,
                &f,
                &s,
                c,
                "s",
                &json!({"searchTerm":"missing","limit":1})
            )),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        c.limits.items = 3;
        assert!(poll(call(op, &f, &s, c, "s", &json!({"limit":1}))).is_ok());
    }
    let count = poll(call(
        "getPlaylistListCount",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({"searchTerm":"o"}),
    ))
    .unwrap();
    let list = poll(call(
        "getPlaylistList",
        &f,
        &s,
        ctx(&s),
        "s",
        &json!({"searchTerm":"o","limit":1}),
    ))
    .unwrap();
    assert_eq!(count, list["totalRecordCount"]);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    for op in ["getPlaylistList", "getGenreList"] {
        assert_eq!(
            poll(call(
                op,
                &f,
                &s,
                ctx(&s),
                "s",
                &json!({"limit":9007199254740992u64})
            )),
            Err(ExchangeError::Malformed)
        );
        assert_eq!(
            poll(call(
                op,
                &f,
                &s,
                ctx(&s),
                "s",
                &json!({"startIndex":9007199254740992u64})
            )),
            Err(ExchangeError::Malformed)
        );
    }
    for op in ["getPlaylistList", "getGenreList", "getPlaylistListCount"] {
        let bad = fake(json!({}));
        assert_eq!(
            poll(call(
                op,
                &bad,
                &s,
                ctx(&s),
                "s",
                &json!({"searchTerm":"x".repeat(8193)})
            )),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            assert_eq!(
                poll(call(op, &bad, &s, ctx(&s), &server, &json!({}))),
                Err(ExchangeError::Malformed)
            );
        }
        assert!(bad.urls.borrow().is_empty());
    }
}

#[test]
fn invalid_input_limits_and_correlated_wire_errors() {
    let s = state();
    for op in ["getPlaylistList", "getPlaylistListCount", "getGenreList"] {
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
    for op in ["getPlaylistList", "getPlaylistListCount", "getGenreList"] {
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
                    json!({"playlists":3,"genres":3})
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
}
