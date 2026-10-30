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
fn query() -> AlbumListQuery {
    ListQuery {
        server: ServerId::new("server-a"),
        sort_by: AlbumListSort::Name,
        sort_order: SortOrder::Asc,
        start_index: 0,
        limit: None,
        search_term: None,
        filter: AlbumFilter::default(),
    }
}
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
fn list(
    f: &dyn ProtocolTransport,
    clock: &dyn Clock,
    c: OperationContext<'_>,
    q: &AlbumListQuery,
    r: &mut Rng,
) -> Result<Page<Album>, ExchangeError> {
    run(get_albums(
        f,
        clock,
        c,
        &ServerBase::parse("http://fixture.invalid/music").unwrap(),
        &Auth::legacy("dummy", "dummy").unwrap(),
        q,
        &mut AlbumReadEnvironment {
            calendar_year: 2026,
            random: r,
        },
    ))
}
fn album(id: &str, name: &str) -> Value {
    json!({"id":id,"name":name,"artist":"Synthetic artist","created":"raw-created","duration":0})
}
#[test]
fn branches_preserve_unknown_and_observed_totals_and_local_shuffle() {
    let s = state();
    let f = fake(vec![reply(
        json!({"albumList2":{"album":[album("b","B"),album("a","A")]}}),
    )]);
    let mut r = rng();
    let mut q = query();
    q.start_index = 2;
    q.limit = Some(1);
    let p = list(&f, &s, ctx(&s), &q, &mut r).unwrap();
    assert_eq!(p.total_record_count, None);
    assert_eq!(p.start_index, 2);
    assert_eq!(p.items.len(), 2);
    assert_eq!(r.calls.get(), 0);
    q.filter.artist_ids = Some(vec![ArtistId::new("a")]);
    q.sort_by = AlbumListSort::Name;
    q.sort_order = SortOrder::Desc;
    let f = fake(vec![reply(
        json!({"artist":{"album":[album("a","A"),album("b","B")]}}),
    )]);
    let p = list(&f, &s, ctx(&s), &q, &mut r).unwrap();
    assert_eq!(p.start_index, 0);
    assert_eq!(p.total_record_count, Some(2));
    assert_eq!(p.items[0].id.as_str(), "b");
    q.filter.artist_ids = None;
    q.filter.favorite = Some(true);
    q.start_index = 0;
    q.limit = None;
    q.sort_by = AlbumListSort::Random;
    let f = fake(vec![reply(
        json!({"starred":{"album":[album("a","A"),album("b","B"),album("c","C")]}}),
    )]);
    let p = list(&f, &s, ctx(&s), &q, &mut r).unwrap();
    assert_eq!(p.total_record_count, Some(3));
    assert_eq!(r.calls.get(), 3);
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

fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-album-lists.json")).unwrap()
}
fn strings<I>(q: &Value, key: &str, build: impl Fn(&str) -> I) -> Option<Vec<I>> {
    q.get(key).filter(|v| !v.is_null()).map(|v| {
        if let Some(s) = v.as_str() {
            vec![build(s)]
        } else {
            v.as_array()
                .unwrap()
                .iter()
                .map(|v| build(v.as_str().unwrap()))
                .collect()
        }
    })
}
fn fixture_query(q: &Value) -> AlbumListQuery {
    ListQuery {
        server: ServerId::new("server-a"),
        sort_by: AlbumListSort::ALL
            .iter()
            .copied()
            .find(|s| Some(s.as_str()) == q["sortBy"].as_str())
            .unwrap(),
        sort_order: if q["sortOrder"] == "DESC" {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        },
        start_index: q["startIndex"].as_u64().unwrap_or(0),
        limit: q["limit"].as_u64(),
        search_term: q["searchTerm"].as_str().map(str::to_owned),
        filter: AlbumFilter {
            artist_ids: strings(q, "artistIds", |s| ArtistId::new(s)),
            genre_ids: strings(q, "genreIds", |s| GenreId::new(s)),
            music_folder_ids: strings(q, "musicFolderId", |s| FolderId::new(s)),
            favorite: q["favorite"].as_bool(),
            min_year: q["minYear"].as_i64().map(|n| n.try_into().unwrap()),
            max_year: q["maxYear"].as_i64().map(|n| n.try_into().unwrap()),
            compilation: q["compilation"].as_bool(),
            has_rating: q["hasRating"].as_bool(),
            is_recently_played: q["isRecentlyPlayed"].as_bool(),
        },
    }
}
#[test]
fn original_controller_sorts_pages_parameters_and_deterministic_rng() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let q = fixture_query(&case["query"]);
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
        let mut r = Rng {
            draws: case
                .get("random_draws")
                .unwrap_or(&fixture["random_draws"])
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().parse().unwrap())
                .collect(),
            calls: Rc::new(Cell::new(0)),
        };
        let result = run(get_albums(
            &f,
            &s,
            ctx(&s),
            &ServerBase::parse("http://fixture.invalid/music").unwrap(),
            &Auth::legacy("dummy", "dummy").unwrap(),
            &q,
            &mut AlbumReadEnvironment {
                calendar_year: case["current_year"].as_i64().unwrap().try_into().unwrap(),
                random: &mut r,
            },
        ));
        if case["target_error"] == "Http" {
            assert!(
                matches!(result, Err(ExchangeError::Http(_))),
                "{}",
                case["name"]
            );
        } else if case["target_error"] == "Malformed" || case.get("expected_error").is_some() {
            assert_eq!(result, Err(ExchangeError::Malformed), "{}", case["name"]);
        } else {
            let p = result.unwrap();
            let actual = json!({"items":p.items.iter().map(project_album).collect::<Vec<_>>(),"startIndex":p.start_index,"totalRecordCount":p.total_record_count});
            let mut expected = case["expected"].clone();
            for a in expected["items"].as_array_mut().unwrap() {
                project_expected(a);
            }
            if let Some(order) = case["target_order"].as_array() {
                assert_eq!(case["name"], "invalid-date-ties-DESC");
                assert_eq!(order, &vec![json!("bad0"), json!("bad1"), json!("good")]);
                expected["items"]
                    .as_array_mut()
                    .unwrap()
                    .sort_by_key(|v| order.iter().position(|id| id == &v["id"]).unwrap());
            }
            projections::equivalent(&actual, &expected);
        }
        assert_eq!(
            r.calls.get(),
            case["rng_calls"].as_u64().unwrap() as usize,
            "{}",
            case["name"]
        );
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

fn empty_body() -> Value {
    json!({"albumList2":{"album":[]},"searchResult3":{"album":[]},"starred":{"album":[]},"artist":{"album":[]}})
}
fn branches() -> Vec<AlbumListQuery> {
    let general = query();
    let mut search = query();
    search.search_term = Some("x".into());
    let mut artists = query();
    artists.filter.artist_ids = Some(vec![ArtistId::new("a")]);
    let mut favorite = query();
    favorite.filter.favorite = Some(true);
    vec![general, search, artists, favorite]
}
#[test]
fn aggregate_bounds_duplicate_albums_nested_song_scope_and_unknown_header_total() {
    let s = state();
    let song = json!({"id":"track","title":"Synthetic","contentType":"audio/flac","created":"raw","size":0,"genres":[{"name":"G"}]});
    let mut a = album("a", "A");
    a["genres"] = json!([{"name":"G"}]);
    a["song"] = json!([song.clone(), song]);
    let mut q = query();
    q.server = ServerId::new("other-server");
    let mut c = ctx(&s);
    c.limits.items = 6;
    let f = fake(vec![reply(json!({"albumList2":{"album":[a.clone()]}}))]);
    let mut r = rng();
    assert_eq!(
        list(&f, &s, c, &q, &mut r),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    c.limits.items = 7;
    let f = fake(vec![Reply {
        header: Some("99999".into()),
        ..reply(json!({"albumList2":{"album":[a]}}))
    }]);
    let p = list(&f, &s, c, &q, &mut r).unwrap();
    assert_eq!(p.total_record_count, None);
    assert_eq!(p.items[0].server, q.server);
    let songs = p.items[0].songs.as_ref().unwrap();
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].id, songs[1].id);
    assert!(songs.iter().all(|song| song.server == q.server
        && song.genres.iter().all(|g| g.server == q.server)
        && song.playlist_item_id.is_none()));
    q.filter.artist_ids = Some(vec![ArtistId::new("a"); 2]);
    let f = fake(vec![
        reply(json!({"artist":{"album":[album("a","A")]}})),
        reply(json!({"artist":{"album":[album("a","A")]}})),
    ]);
    c.limits.items = 4;
    assert_eq!(
        list(&f, &s, c, &q, &mut r),
        Err(ExchangeError::Context(DomainError::Capacity))
    );
    assert_eq!(f.urls.borrow().len(), 2);
    let f = fake(vec![
        reply(json!({"artist":{"album":[album("a","A")]}})),
        reply(json!({"artist":{"album":[album("a","A")]}})),
    ]);
    c.limits.items = 5;
    let p = list(&f, &s, c, &q, &mut r).unwrap();
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.items[0].id, p.items[1].id);
    assert_eq!(p.total_record_count, Some(2));
}
#[test]
fn every_branch_input_limits_wire_errors_and_terminal_partial_failure() {
    let s = state();
    for q in branches() {
        for server in ["".to_owned(), " ".to_owned(), "x".repeat(129)] {
            let f = fake(vec![]);
            let mut q = q.clone();
            q.server = ServerId::new(server);
            let mut r = rng();
            assert_eq!(
                list(&f, &s, ctx(&s), &q, &mut r),
                Err(ExchangeError::Malformed)
            );
            assert!(f.urls.borrow().is_empty());
            assert_eq!(r.calls.get(), 0);
        }
        for mode in 0..4 {
            let f = fake(vec![]);
            let mut q = q.clone();
            let mut c = ctx(&s);
            match mode {
                0 => q.limit = Some(9007199254740992),
                1 => q.start_index = 9007199254740992,
                2 => q.search_term = Some("x".repeat(8193)),
                _ => c.limits.items = 0,
            };
            assert!(list(&f, &s, c, &q, &mut rng()).is_err());
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
                list(&f, &s, c, &q, &mut rng()),
                Err(ExchangeError::Context(DomainError::Capacity))
            );
        }
        let f = fake(vec![Reply {
            stale: true,
            ..reply(empty_body())
        }]);
        assert_eq!(
            list(&f, &s, ctx(&s), &q, &mut rng()),
            Err(ExchangeError::Stale)
        );
        let f = fake(vec![Reply {
            error: Some(TransportError::Network),
            ..reply(empty_body())
        }]);
        assert_eq!(
            list(&f, &s, ctx(&s), &q, &mut rng()),
            Err(ExchangeError::Transport(TransportError::Network))
        );
        let f = fake(vec![reply(
            json!({"status":"failed","error":{"code":40,"message":"private-marker"}}),
        )]);
        let e = list(&f, &s, ctx(&s), &q, &mut rng()).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(ProtocolError::Authentication));
        assert!(!format!("{e:?}").contains("private-marker"));
        let f = fake(vec![Reply {
            status: 401,
            ..reply(empty_body())
        }]);
        let result = list(&f, &s, ctx(&s), &q, &mut rng());
        if q.filter.artist_ids.is_some() {
            assert!(result.unwrap().items.is_empty());
        } else {
            assert_eq!(result, Err(ExchangeError::Http(HttpError::Authentication)));
        }
    }
    let mut q = query();
    q.filter.artist_ids = Some(vec![ArtistId::new("a"), ArtistId::new("b")]);
    let f = fake(vec![
        reply(json!({"artist":{"album":[album("a","A")]}})),
        Reply {
            stale: true,
            ..reply(empty_body())
        },
    ]);
    assert_eq!(
        list(&f, &s, ctx(&s), &q, &mut rng()),
        Err(ExchangeError::Stale)
    );
    for which in 0..3 {
        let f = fake(vec![]);
        let mut q = query();
        let mut c = ctx(&s);
        c.limits.items = 1;
        match which {
            0 => q.filter.artist_ids = Some(vec![ArtistId::new("a"); 2]),
            1 => q.filter.genre_ids = Some(vec![GenreId::new("g"); 2]),
            _ => q.filter.music_folder_ids = Some(vec![FolderId::new("f"); 2]),
        };
        assert_eq!(
            list(&f, &s, c, &q, &mut rng()),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        assert!(f.urls.borrow().is_empty());
    }
}
#[test]
fn pending_and_final_context_priority_including_empty_artist_selection() {
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
    for q in branches() {
        for cancel in [false, true] {
            let s = state();
            let base = ServerBase::parse("http://fixture.invalid").unwrap();
            let auth = Auth::legacy("dummy", "dummy").unwrap();
            let mut r = rng();
            let mut environment = AlbumReadEnvironment {
                calendar_year: 2026,
                random: &mut r,
            };
            let mut future = Box::pin(get_albums(
                &Pending,
                &s,
                ctx(&s),
                &base,
                &auth,
                &q,
                &mut environment,
            ));
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
                    state: &s,
                    cancel,
                    after: 5,
                };
                let f = fake(vec![reply(if malformed {
                    json!({"albumList2":3,"artist":3,"starred":3,"searchResult3":3})
                } else {
                    empty_body()
                })]);
                assert_eq!(
                    list(&f, &clock, ctx(&s), &q, &mut rng()),
                    Err(ExchangeError::Context(if cancel {
                        DomainError::Cancelled
                    } else {
                        DomainError::Deadline
                    }))
                );
            }
        }
    }
    let s = state();
    let clock = Late {
        calls: Cell::new(0),
        state: &s,
        cancel: false,
        after: 1,
    };
    let f = fake(vec![]);
    let mut q = query();
    q.filter.artist_ids = Some(vec![]);
    assert_eq!(
        list(&f, &clock, ctx(&s), &q, &mut rng()),
        Err(ExchangeError::Context(DomainError::Deadline))
    );
    assert!(f.urls.borrow().is_empty());
}
