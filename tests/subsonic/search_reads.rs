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
fn project_album(a: &Album) -> Value {
    let participants = a.participants.as_ref().map(|p| {
        p.iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!(v.iter().map(projections::related).collect::<Vec<_>>()),
                )
            })
            .collect::<serde_json::Map<_, _>>()
    });
    json!({"_itemType":"album","_serverId":a.server.as_str(),"_serverType":server_type(a.server_type),"albumArtistName":a.album_artist_name,"albumArtists":a.album_artists.iter().map(projections::related).collect::<Vec<_>>(),"artists":a.artists.iter().map(projections::related).collect::<Vec<_>>(),"blurHash":a.blur_hash,"comment":a.comment,"createdAt":a.created_at,"discs":a.discs,"dominantColor":a.dominant_color,"duration":a.duration.map(|v|v.0),"explicitStatus":a.explicit_status.map(|v|match v {ExplicitStatus::Explicit=>"EXPLICIT",ExplicitStatus::Clean=>"CLEAN"}),"gain":projections::gain(&a.gain),"genres":a.genres.iter().map(genre).collect::<Vec<_>>(),"id":a.id.as_str(),"imageId":a.image_id,"imageUrl":a.image_url,"isCompilation":a.is_compilation,"lastPlayedAt":a.last_played_at,"mbzId":a.mbz_id,"mbzReleaseGroupId":a.mbz_release_group_id,"missing":a.missing,"name":a.name,"originalDate":a.original_date,"originalYear":a.original_year,"participants":participants,"peak":projections::gain(&a.peak),"playCount":a.play_count,"ratedAt":a.rated_at,"recordLabels":a.record_labels,"releaseDate":a.release_date,"releaseType":a.release_type,"releaseTypes":a.release_types,"releaseYear":a.release_year,"size":a.size,"songCount":a.song_count,"songs":a.songs.as_ref().map(|songs|songs.iter().map(projections::song).collect::<Vec<_>>()),"sortName":a.sort_name,"starredAt":a.starred_at,"tags":a.tags,"thumbHash":a.thumb_hash,"trackYearRange":a.track_year_range.as_ref().map(|v|json!({"min":v.min,"max":v.max})),"updatedAt":a.updated_at,"userFavorite":a.user_favorite,"userRating":a.user_rating,"version":a.version})
}
fn artist(a: &AlbumArtist) -> Value {
    assert_eq!(a.server_type, ServerType::Subsonic);
    assert_eq!(a.uploaded_image, None);
    let v = json!({"_itemType":"albumArtist","_serverId":a.server.as_str(),"_serverType":server_type(a.server_type),"albumCount":a.album_count,"biography":a.biography,"blurHash":a.blur_hash,"dominantColor":a.dominant_color,"duration":a.duration.map(|v|v.0),"genres":a.genres.iter().map(genre).collect::<Vec<_>>(),"id":a.id.as_str(),"imageId":a.image_id,"imageUrl":a.image_url,"lastPlayedAt":a.last_played_at,"mbz":a.mbz,"missing":a.missing,"name":a.name,"playCount":a.play_count,"ratedAt":a.rated_at,"similarArtists":a.similar_artists.as_ref().map(|v|v.iter().map(projections::related).collect::<Vec<_>>()),"songCount":a.song_count,"starredAt":a.starred_at,"thumbHash":a.thumb_hash,"userFavorite":a.user_favorite,"userRating":a.user_rating});

    v
}
fn query(v: &Value) -> SearchQuery {
    SearchQuery {
        server: ServerId::new("server-a"),
        album_artist_limit: v["albumArtistLimit"].as_u64(),
        album_artist_start_index: v["albumArtistStartIndex"].as_u64(),
        album_limit: v["albumLimit"].as_u64(),
        album_start_index: v["albumStartIndex"].as_u64(),
        song_limit: v["songLimit"].as_u64(),
        song_start_index: v["songStartIndex"].as_u64(),
        query: v["query"].as_str().map(str::to_owned),
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
    }
}
fn read(
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    q: &SearchQuery,
) -> Result<SearchResults, ExchangeError> {
    run(search(
        f,
        clock,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        q,
    ))
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
#[test]
fn pinned_combined_search_preserves_every_field_and_independent_family_windows() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/p03/subsonic-search.json")).unwrap();
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
        let result = read(&f, &s, ctx(&s), &query(&case["query"]));
        match case["target_error"].as_str() {
            Some("Malformed") => {
                assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"])
            }
            Some("Http") => assert!(
                matches!(result, Err(ExchangeError::Http(_))),
                "{}",
                case["name"]
            ),
            Some("Protocol") => assert_eq!(
                result,
                Err(ExchangeError::Protocol(ProtocolError::Authentication))
            ),
            None => {
                let result = result.unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
                let actual = json!({"albumArtists":result.album_artists.iter().map(artist).collect::<Vec<_>>(),"albums":result.albums.iter().map(project_album).collect::<Vec<_>>(),"songs":result.songs.iter().map(projections::song).collect::<Vec<_>>()});
                let mut expected = case["expected"].clone();
                project_expected(&mut expected);
                projections::equivalent(&actual, &expected);
            }
            _ => panic!("unknown target error"),
        }
        let urls = f.urls.borrow();
        let requests = case["requests"].as_array().unwrap();
        assert_eq!(urls.len(), requests.len());
        for (url, request) in urls.iter().zip(requests) {
            let url = url::Url::parse(url).unwrap();
            assert_eq!(url.path(), "/music/rest/search3.view");
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
    json!({"id":"same","title":"Song","created":"raw","contentType":"audio/flac","size":1})
}
#[test]
fn cross_family_nested_work_is_aggregate_and_server_scoped() {
    let s = state();
    let mut q = query(&json!({"albumArtistLimit":0,"albumLimit":0,"songLimit":0}));
    q.server = ServerId::new("other");
    let body = json!({"searchResult3":{"artist":[{"id":"a","name":"A","similarArtists":[{"id":"peer","name":"Peer"}]}],"album":[{"id":"al","name":"Album","artist":"A","created":"raw","duration":1,"song":[song(),song()]}],"song":[song(),song()]}});
    // root + artist + similar artist + album + 2 nested songs + 2 root songs = 8.
    let f = fake(vec![reply(body.clone())]);
    let mut c = ctx(&s);
    c.limits.items = 7;
    assert_eq!(
        read(&f, &s, c, &q),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    let f = fake(vec![reply(body)]);
    c.limits.items = 8;
    let result = read(&f, &s, c, &q).unwrap();
    assert_eq!(result.songs.len(), 2);
    assert_eq!(result.songs[0].id, result.songs[1].id);
    assert_eq!(result.albums[0].songs.as_ref().unwrap().len(), 2);
    assert_eq!(result.album_artists[0].server, q.server);
    assert_eq!(result.albums[0].server, q.server);
    assert!(
        result
            .songs
            .iter()
            .chain(result.albums[0].songs.as_ref().unwrap())
            .all(|s| s.server == q.server && s.playlist_item_id.is_none())
    );
    let f = fake(vec![reply(json!({"searchResult3":{"song":[song(),null]}}))]);
    assert_eq!(read(&f, &s, ctx(&s), &q), Err(ExchangeError::Malformed));
}
#[test]
fn every_window_and_scoped_input_is_checked_before_dispatch() {
    let s = state();
    let f = fake(vec![]);
    for field in [
        "albumArtistLimit",
        "albumArtistStartIndex",
        "albumLimit",
        "albumStartIndex",
        "songLimit",
        "songStartIndex",
    ] {
        let mut v = json!({});
        v[field] = json!(9007199254740992u64);
        assert_eq!(
            read(&f, &s, ctx(&s), &query(&v)),
            Err(ExchangeError::Malformed)
        );
    }
    for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
        let mut q = query(&json!({}));
        q.server = ServerId::new(server);
        assert_eq!(read(&f, &s, ctx(&s), &q), Err(ExchangeError::Malformed));
    }
    let mut c = ctx(&s);
    c.limits.items = 1;
    assert_eq!(
        read(&f, &s, c, &query(&json!({"musicFolderId":["a","b"]}))),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(
        read(&f, &s, ctx(&s), &query(&json!({"query":"x".repeat(8193)}))),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert!(f.urls.borrow().is_empty());
}
#[test]
fn errors_bytes_and_stale_responses_cannot_publish_partial_families() {
    let s = state();
    let q = query(&json!({}));
    for request in [true, false] {
        let f = fake(vec![reply(json!({}))]);
        let mut c = ctx(&s);
        if request {
            c.limits.request_bytes = 1;
        } else {
            c.limits.response_bytes = 1;
        }
        assert_eq!(
            read(&f, &s, c, &q),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        assert_eq!(f.urls.borrow().len(), usize::from(!request));
    }
    let f = fake(vec![Reply {
        stale: true,
        ..reply(json!({}))
    }]);
    assert_eq!(read(&f, &s, ctx(&s), &q), Err(ExchangeError::Stale));
    let f = fake(vec![Reply {
        error: Some(TransportError::Network),
        ..reply(json!({}))
    }]);
    assert_eq!(
        read(&f, &s, ctx(&s), &q),
        Err(ExchangeError::Transport(TransportError::Network))
    );
    let f = fake(vec![reply(
        json!({"status":"failed","error":{"code":40,"message":"private-marker"}}),
    )]);
    let e = read(&f, &s, ctx(&s), &q).unwrap_err();
    assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
    assert!(!format!("{e:?}").contains("private-marker"));
}
#[test]
fn pending_and_final_context_priority_applies_to_success_and_malformed() {
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
    for cancel in [false, true] {
        let s = state();
        let base = ServerBase::parse("http://fixture.invalid").unwrap();
        let auth = Auth::legacy("dummy", "dummy").unwrap();
        let q = query(&json!({}));
        let mut future = Box::pin(search(&Pending, &s, ctx(&s), &base, &auth, &q));
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
                json!({"searchResult3":3})
            } else {
                json!({})
            })]);
            assert_eq!(read(&f, &clock, ctx(&s), &q), Err(error));
        }
    }
}
