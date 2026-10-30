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
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-artist-lists.json")).unwrap()
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

fn server_type(value: ServerType) -> &'static str {
    match value {
        ServerType::Subsonic => "subsonic",
        ServerType::Navidrome => "navidrome",
        ServerType::Jellyfin => "jellyfin",
    }
}
fn genre(g: &Genre) -> Value {
    json!({"_itemType":"genre","_serverId":g.server.as_str(),"_serverType":server_type(g.server_type),"albumCount":g.album_count,"id":g.id.as_str(),"imageId":g.image_id,"imageUrl":g.image_url,"name":g.name,"songCount":g.song_count})
}
fn artist(a: &AlbumArtist) -> Value {
    assert_eq!(a.server_type, ServerType::Subsonic);
    assert_eq!(a.uploaded_image, None);
    let v = json!({"_itemType":"albumArtist","_serverId":a.server.as_str(),"_serverType":server_type(a.server_type),"albumCount":a.album_count,"biography":a.biography,"blurHash":a.blur_hash,"dominantColor":a.dominant_color,"duration":a.duration.map(|v|v.0),"genres":a.genres.iter().map(genre).collect::<Vec<_>>(),"id":a.id.as_str(),"imageId":a.image_id,"imageUrl":a.image_url,"lastPlayedAt":a.last_played_at,"mbz":a.mbz,"missing":a.missing,"name":a.name,"playCount":a.play_count,"ratedAt":a.rated_at,"similarArtists":a.similar_artists.as_ref().map(|v|v.iter().map(projections::related).collect::<Vec<_>>()),"songCount":a.song_count,"starredAt":a.starred_at,"thumbHash":a.thumb_hash,"userFavorite":a.user_favorite,"userRating":a.user_rating});

    v
}

fn folders(q: &Value) -> Option<Vec<FolderId>> {
    q.get("musicFolderId").map(|v| {
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
    let sort_order = if q["sortOrder"] == "DESC" {
        SortOrder::Desc
    } else {
        SortOrder::Asc
    };
    let start_index = q["startIndex"].as_u64().unwrap_or(0);
    let limit = q["limit"].as_u64();
    let search_term = q["searchTerm"].as_str().map(str::to_owned);
    if op.starts_with("getAlbumArtist") {
        let query = ListQuery {
            server,
            sort_by: AlbumArtistListSort::ALL
                .iter()
                .copied()
                .find(|s| Some(s.as_str()) == q["sortBy"].as_str())
                .unwrap_or(AlbumArtistListSort::Name),
            sort_order,
            start_index,
            limit,
            search_term,
            filter: AlbumArtistFilter {
                favorite: q["favorite"].as_bool(),
                music_folder_ids: folders(q),
            },
        };
        if op == "getAlbumArtistListCount" {
            return get_album_artist_count(f, clock, c, &base, &auth, &query)
                .await
                .map(|n| json!(n));
        }
        get_album_artists(f,clock,c,&base,&auth,&query).await.map(|p|json!({"items":p.items.iter().map(artist).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count}))
    } else {
        let query = ListQuery {
            server,
            sort_by: ArtistListSort::ALL
                .iter()
                .copied()
                .find(|s| Some(s.as_str()) == q["sortBy"].as_str())
                .unwrap_or(ArtistListSort::Name),
            sort_order,
            start_index,
            limit,
            search_term,
            filter: ArtistFilter {
                favorite: q["favorite"].as_bool(),
                music_folder_ids: folders(q),
                role: q["role"].as_str().map(str::to_owned),
            },
        };
        if op == "getArtistListCount" {
            return get_artist_count(f, clock, c, &base, &auth, &query)
                .await
                .map(|n| json!(n));
        }
        get_artists(f,clock,c,&base,&auth,&query).await.map(|p|json!({"items":p.items.iter().map(|a|artist(&a.0)).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count}))
    }
}
fn project(v: &mut Value) {
    if let Some(items) = v.get_mut("items").and_then(Value::as_array_mut) {
        for a in items {
            if let Some(similar) = a.get_mut("similarArtists").and_then(Value::as_array_mut) {
                for a in similar {
                    if a["imageId"].is_number() {
                        a["imageId"] = json!(a["imageId"].to_string());
                    }
                }
            }
        }
    }
}
#[test]
fn source_lists_counts_all_sort_variants_and_filter_order() {
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
        assert_eq!(url.path(), "/music/rest/getArtists.view");
        let actual: Vec<_> = url
            .query_pairs()
            .filter(|(k, _)| !["c", "f", "v", "u", "p"].contains(&k.as_ref()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let expected: Vec<_> = case["request"]["args"]["query"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        assert_eq!(actual, expected);
    }
}
#[test]
fn work_counts_excluded_rows_roles_and_similar_metadata() {
    let s = state();
    let f = fake(
        json!({"artists":{"index":[{"artist":[{"id":"a","name":"A","roles":[]},{"id":"b","name":"B","roles":["vocal","vocal"],"similarArtists":[{"id":"c","name":"C"}]}]}]}}),
    );
    let mut c = ctx(&s);
    c.limits.items = 6;
    assert_eq!(
        poll(call(
            "getArtistList",
            &f,
            &s,
            c,
            "other-server",
            &json!({"role":"vocal","limit":1})
        )),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 7;
    let page = poll(call(
        "getArtistList",
        &f,
        &s,
        c,
        "other-server",
        &json!({"role":"vocal","limit":1}),
    ))
    .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["_serverId"], "other-server");
    assert_eq!(page["items"][0]["similarArtists"][0]["id"], "c");
    for op in [
        "getAlbumArtistList",
        "getAlbumArtistListCount",
        "getArtistList",
        "getArtistListCount",
    ] {
        c.limits.items = 3;
        assert_eq!(
            poll(call(
                op,
                &f,
                &s,
                c,
                "s",
                &json!({"searchTerm":"missing","startIndex":99,"limit":1})
            )),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        for q in [
            json!({"limit":9007199254740992u64}),
            json!({"searchTerm":"x".repeat(8193)}),
            json!({"musicFolderId":["a","b","c","d"]}),
        ] {
            let bad = fake(json!({}));
            assert!(poll(call(op, &bad, &s, c, "s", &q)).is_err());
            assert!(bad.urls.borrow().is_empty());
        }
    }
    let bad = fake(json!({}));
    assert_eq!(
        poll(call(
            "getArtistList",
            &bad,
            &s,
            ctx(&s),
            "s",
            &json!({"role":"x".repeat(8193)})
        )),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert!(bad.urls.borrow().is_empty());
}

#[test]
fn invalid_input_limits_and_correlated_wire_errors() {
    let s = state();
    for op in [
        "getAlbumArtistList",
        "getAlbumArtistListCount",
        "getArtistList",
        "getArtistListCount",
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
    for op in [
        "getAlbumArtistList",
        "getAlbumArtistListCount",
        "getArtistList",
        "getArtistListCount",
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
                    after: 5,
                    calls: Cell::new(0),
                    s: &s,
                    cancel,
                };
                let body = if malformed {
                    json!({"artists":3})
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
    for op in [
        "getArtistList",
        "getArtistListCount",
        "getAlbumArtistListCount",
    ] {
        let s = state();
        let clock = Late {
            after: 6,
            calls: Cell::new(0),
            s: &s,
            cancel: false,
        };
        let f = fake(json!({}));
        assert_eq!(
            poll(call(op, &f, &clock, ctx(&s), "s", &json!({}))),
            Err(ExchangeError::Context(DomainError::Deadline))
        );
    }
}
