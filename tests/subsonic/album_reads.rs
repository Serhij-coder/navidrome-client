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
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-album-reads.json")).unwrap()
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
fn album_input() -> Value {
    fixture()["cases"][0]["input"].clone()
}
fn read_album(
    f: &dyn ProtocolTransport,
    s: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    id: &str,
) -> Result<Album, ExchangeError> {
    poll(get_album(
        f,
        s,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        &Scoped {
            server: ServerId::new(server),
            id: AlbumId::new(id),
        },
    ))
}
fn read_artist(
    f: &dyn ProtocolTransport,
    s: &dyn Clock,
    c: OperationContext<'_>,
    server: &str,
    id: &str,
) -> Result<AlbumArtistDetail, ExchangeError> {
    poll(get_album_artist(
        f,
        s,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        &Scoped {
            server: ServerId::new(server),
            id: ArtistId::new(id),
        },
    ))
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
fn album(a: &Album) -> Value {
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
fn artist(d: &AlbumArtistDetail) -> Value {
    let a = &d.artist;
    assert_eq!(a.uploaded_image, None);
    let mut v = json!({"_itemType":"albumArtist","_serverId":a.server.as_str(),"_serverType":server_type(a.server_type),"albumCount":a.album_count,"biography":a.biography,"blurHash":a.blur_hash,"dominantColor":a.dominant_color,"duration":a.duration.map(|v|v.0),"genres":a.genres.iter().map(genre).collect::<Vec<_>>(),"id":a.id.as_str(),"imageId":a.image_id,"imageUrl":a.image_url,"lastPlayedAt":a.last_played_at,"mbz":a.mbz,"missing":a.missing,"name":a.name,"playCount":a.play_count,"ratedAt":a.rated_at,"similarArtists":a.similar_artists.as_ref().map(|v|v.iter().map(projections::related).collect::<Vec<_>>()),"songCount":a.song_count,"starredAt":a.starred_at,"thumbHash":a.thumb_hash,"userFavorite":a.user_favorite,"userRating":a.user_rating});
    if let Some(albums) = &d.albums {
        v["albums"] = json!(albums.iter().map(album).collect::<Vec<_>>());
    }
    v
}
fn project_expected(v: &mut Value) {
    if let Some(items) = v.get_mut("albums").and_then(Value::as_array_mut) {
        for item in items {
            project_expected(item);
        }
    }
    if v["_itemType"] == "album" {
        if v.get("sortName").is_none_or(Value::is_null) {
            v["sortName"] = v["name"].clone();
        }
        v["duration"] = json!(v["duration"].as_f64().unwrap() as u64);
        if v.get("songCount").is_none() {
            v["songCount"] = Value::Null;
        }
        for song in v["songs"].as_array_mut().unwrap() {
            song["duration"] = json!(song["duration"].as_f64().unwrap() as u64);
            song["playlistItemId"] = Value::Null;
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
#[test]
fn pinned_controller_and_all_normalized_fields() {
    for case in fixture()["cases"].as_array().unwrap() {
        let s = state();
        let kind = if case["kind"] == "album" {
            "album"
        } else {
            "artist"
        };
        let f = fake(kind, case["input"].clone());
        let id = case["input"]["id"].as_str().unwrap();
        let actual = if kind == "album" {
            read_album(&f, &s, ctx(&s), "server-a", id).map(|a| album(&a))
        } else {
            read_artist(&f, &s, ctx(&s), "server-a", id).map(|a| artist(&a))
        };
        if case.get("expected_error").is_some() || case["target_error"] == true {
            assert_eq!(actual, Err(ExchangeError::Malformed), "{}", case["name"]);
            continue;
        }
        let mut expected = case["expected"].clone();
        project_expected(&mut expected);
        projections::equivalent(&actual.unwrap(), &expected);
        assert!(f.urls.borrow()[0].contains(if kind == "album" {
            "/rest/getAlbum.view?"
        } else {
            "/rest/getArtist.view?"
        }));
        assert!(f.urls.borrow()[0].ends_with(&format!("id={id}")));
    }
}
#[test]
fn nested_work_budget_disc_lookup_and_server_scope() {
    let s = state();
    let rich = fixture()["cases"][1]["input"].clone();
    let f = fake("album", rich);
    let value = read_album(&f, &s, ctx(&s), "other-server", "album-1").unwrap();
    let songs = value.songs.as_ref().unwrap();
    assert_eq!(songs[0].id, songs[1].id);
    assert_eq!(songs[0].disc_subtitle.as_deref(), Some("new"));
    assert_eq!(songs[1].disc_number, 1);
    assert_eq!(songs[1].disc_subtitle.as_deref(), Some("zero"));
    assert!(
        songs.iter().all(|v| v.server.as_str() == "other-server"
            && v.genres.iter().all(|g| g.server == v.server))
    );
    let base = album_input();
    let f = fake(
        "artist",
        json!({"id":"artist-1","name":"A","album":[base.clone(),base]}),
    );
    let mut c = ctx(&s);
    c.limits.items = 2;
    assert_eq!(
        read_artist(&f, &s, c, "s", "artist-1"),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 3;
    assert!(read_artist(&f, &s, c, "s", "artist-1").is_ok());
    let f = fake("album", album_input());
    c.limits.items = 1;
    assert!(read_album(&f, &s, c, "s", "album-1").is_ok());
    c.limits.items = 0;
    let before = f.urls.borrow().len();
    assert_eq!(
        read_album(&f, &s, c, "s", "album-1"),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), before);
}
#[test]
fn invalid_values_fail_closed_and_context_is_guarded() {
    let s = state();
    let f = fake("album", album_input());
    for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
        assert_eq!(
            read_album(&f, &s, ctx(&s), &server, "album-1"),
            Err(ExchangeError::Malformed)
        );
    }
    assert!(f.urls.borrow().is_empty());
    for (key, value) in [
        ("duration", json!(-1)),
        ("name", json!(null)),
        ("discTitles", json!([{"disc":4294967296u64,"title":"bad"}])),
        ("song", json!([null])),
    ] {
        let mut raw = album_input();
        raw[key] = value;
        let f = fake("album", raw);
        assert_eq!(
            read_album(&f, &s, ctx(&s), "s", "album-1"),
            Err(ExchangeError::Malformed)
        );
    }
    let f = fake("album", album_input());
    assert_eq!(
        read_album(&f, &s, ctx(&s), "s", "wrong"),
        Err(ExchangeError::Stale)
    );
    let mut c = ctx(&s);
    c.limits.request_bytes = 1;
    assert_eq!(
        read_album(&f, &s, c, "s", "album-1"),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c = ctx(&s);
    c.limits.response_bytes = 1;
    assert_eq!(
        read_album(&f, &s, c, "s", "album-1"),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    s.cancelled.set(true);
    assert_eq!(
        read_album(&f, &s, ctx(&s), "s", "album-1"),
        Err(ExchangeError::Context(DomainError::Cancelled))
    );
    s.cancelled.set(false);
    s.now.set(11);
    assert_eq!(
        read_album(&f, &s, ctx(&s), "s", "album-1"),
        Err(ExchangeError::Context(DomainError::Deadline))
    );
    s.now.set(10);
    assert!(read_album(&f, &s, ctx(&s), "s", "album-1").is_ok());
    for code in [401, 403, 500] {
        let mut f = fake("album", album_input());
        f.status = code;
        assert!(matches!(
            read_album(&f, &s, ctx(&s), "s", "album-1"),
            Err(ExchangeError::Http(_))
        ));
    }
    let mut f = fake("album", album_input());
    f.stale = true;
    assert_eq!(
        read_album(&f, &s, ctx(&s), "s", "album-1"),
        Err(ExchangeError::Stale)
    );
    f.stale = false;
    f.error = Some(TransportError::Network);
    assert_eq!(
        read_album(&f, &s, ctx(&s), "s", "album-1"),
        Err(ExchangeError::Transport(TransportError::Network))
    );
    let f=Fake{body:br#"{"subsonic-response":{"status":"failed","version":"1","error":{"code":40,"message":"private-marker"}}}"#.to_vec(),..fake("album",album_input())};
    let e = read_album(&f, &s, ctx(&s), "s", "album-1").unwrap_err();
    assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
    assert!(!format!("{e:?}").contains("private-marker"));
    let f = fake("artist", json!({"id":"artist-1","name":"A"}));
    assert!(
        read_artist(&f, &s, ctx(&s), "s", "artist-1")
            .unwrap()
            .albums
            .is_none()
    );
}

#[test]
fn pending_and_post_normalization_context_guards_cover_both_reads() {
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
    for artist in [false, true] {
        let s = state();
        let base = ServerBase::parse("http://fixture.invalid").unwrap();
        let auth = Auth::legacy("dummy", "dummy").unwrap();
        let album_id = Scoped {
            server: ServerId::new("s"),
            id: AlbumId::new("album-1"),
        };
        let artist_id = Scoped {
            server: ServerId::new("s"),
            id: ArtistId::new("artist-1"),
        };
        let pending = Pending;
        let mut future: LocalFuture<'_, Result<(), ExchangeError>> = if artist {
            Box::pin(async {
                get_album_artist(&pending, &s, ctx(&s), &base, &auth, &artist_id)
                    .await
                    .map(|_| ())
            })
        } else {
            Box::pin(async {
                get_album(&pending, &s, ctx(&s), &base, &auth, &album_id)
                    .await
                    .map(|_| ())
            })
        };
        let w = Waker::from(Arc::new(Noop));
        let mut cx = Context::from_waker(&w);
        assert!(future.as_mut().poll(&mut cx).is_pending());
        s.cancelled.set(true);
        assert_eq!(
            future.as_mut().poll(&mut cx),
            Poll::Ready(Err(ExchangeError::Context(DomainError::Cancelled)))
        );
        drop(future);
        for cancel in [false, true] {
            for mode in 0..4 {
                let s = state();
                let clock = Late {
                    calls: Cell::new(0),
                    state: &s,
                    cancel,
                };
                let mut input = if artist {
                    json!({"id":"artist-1","name":"A"})
                } else {
                    album_input()
                };
                match mode {
                    0 => input = Value::Null,
                    1 => input["name"] = json!({"private":"marker"}),
                    2 => input["id"] = json!("wrong"),
                    _ => {}
                }
                let f = fake(if artist { "artist" } else { "album" }, input);
                let result = if artist {
                    read_artist(&f, &clock, ctx(&s), "s", "artist-1").map(|_| ())
                } else {
                    read_album(&f, &clock, ctx(&s), "s", "album-1").map(|_| ())
                };
                assert_eq!(
                    result,
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
