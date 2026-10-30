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
struct State {
    cancelled: Cell<bool>,
    now: Cell<u64>,
}
impl Cancellation for State {
    fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }
}
impl Clock for State {
    fn now(&self) -> Milliseconds {
        Milliseconds(self.now.get())
    }
}
fn state() -> State {
    State {
        cancelled: Cell::new(false),
        now: Cell::new(10),
    }
}
fn context(s: &State) -> OperationContext<'_> {
    OperationContext {
        correlation: RequestContext {
            request: RequestId(1),
            generation: Generation(2),
            deadline: Milliseconds(10),
        },
        limits: OperationLimits {
            request_bytes: 10000,
            response_bytes: 100000,
            items: 100,
        },
        cancellation: s,
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
        ctx: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
        assert!(matches!(req.method(), Method::Get));
        assert!(req.expose_body_for_transport().is_none());
        self.urls
            .borrow_mut()
            .push(req.expose_url_for_transport().into());
        Box::pin(async move {
            if let Some(e) = self.error {
                return Err(e);
            }
            let mut correlation = ctx.correlation;
            if self.stale {
                correlation.generation = Generation(3)
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
fn fake(song: Value) -> Fake {
    Fake {
        body: serde_json::to_vec(
            &json!({"subsonic-response":{"status":"ok","version":"1.16.1","song":song}}),
        )
        .unwrap(),
        status: 200,
        stale: false,
        error: None,
        urls: RefCell::new(vec![]),
    }
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
fn run<F: Future>(f: F) -> F::Output {
    let mut f = Box::pin(f);
    let w = Waker::from(Arc::new(Noop));
    match f.as_mut().poll(&mut Context::from_waker(&w)) {
        Poll::Ready(r) => r,
        Poll::Pending => panic!("unexpected pending"),
    }
}
fn identity(id: &str) -> Scoped<SongId> {
    Scoped {
        server: ServerId::new("server-a"),
        id: SongId::new(id),
    }
}
fn read(
    f: &Fake,
    s: &State,
    c: OperationContext<'_>,
    id: &Scoped<SongId>,
) -> Result<Song, ExchangeError> {
    run(get_song(
        f,
        s,
        c,
        &ServerBase::parse("https://example.test/music").unwrap(),
        &Auth::legacy("dummy", "synthetic-secret").unwrap(),
        id,
    ))
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/p03/subsonic-song-reads.json")).unwrap()
}
fn minimal() -> Value {
    json!({"id":"song /1","title":"Title","contentType":"audio/flac","created":"raw","size":10})
}
#[path = "support/projection.rs"]
mod projections;
use projections::{equivalent, song as projection};
#[test]
fn pinned_normalizer_differential_all_fields() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let s = state();
        let f = fake(case["input"].clone());
        if case.get("expected_error").is_some() {
            assert_eq!(
                read(&f, &s, context(&s), &identity("song /1")),
                Err(ExchangeError::Malformed)
            );
            continue;
        }
        let id = case["expected"]["id"].as_str().unwrap();
        let song = read(&f, &s, context(&s), &identity(id)).unwrap();
        assert_eq!(song.server_type, ServerType::Subsonic);
        assert!(
            song.genres
                .iter()
                .all(|g| g.server_type == ServerType::Subsonic)
        );
        assert_eq!(song.is_resumable, None);
        assert_eq!(song.resume_position_ms, None);
        let mut expected = case["expected"].clone();
        expected["playlistItemId"] = Value::Null;
        for key in ["gain", "peak"] {
            if expected[key].is_object() {
                for component in ["album", "track"] {
                    if expected[key].get(component).is_none() {
                        expected[key][component] = Value::Null
                    }
                }
            }
        }
        // Integer domain units deliberately truncate fractional milliseconds.
        expected["duration"] = json!(expected["duration"].as_f64().unwrap() as u64);
        equivalent(&projection(&song), &expected);
        assert!(f.urls.borrow()[0].contains("/music/rest/getSong.view?"));
        assert!(f.urls.borrow()[0].ends_with(&format!(
                "id={}",
                url::form_urlencoded::byte_serialize(id.as_bytes())
                    .collect::<String>()
                    .replace('+', "%20")
            )));
    }
}
#[test]
fn same_song_id_is_scoped_to_selected_server() {
    let s = state();
    let f = fake(minimal());
    let a = read(&f, &s, context(&s), &identity("song /1")).unwrap();
    let mut id = identity("song /1");
    id.server = ServerId::new("server-b");
    let b = read(&f, &s, context(&s), &id).unwrap();
    assert_ne!(a.server, b.server);
    assert_eq!(a.id, b.id);
    let f =
        fake(json!({"id":"other","title":"T","contentType":"audio/flac","created":"r","size":0}));
    assert_eq!(read(&f, &s, context(&s), &id), Err(ExchangeError::Stale));
}
#[test]
fn malformed_wire_and_unrepresentable_numbers_fail_closed() {
    let s = state();
    for (key, value) in [
        ("id", json!({})),
        ("id", json!(1.5)),
        ("id", json!(9007199254740992u64)),
        ("title", json!(null)),
        ("contentType", json!(false)),
        ("created", json!(4)),
        ("size", json!(-1)),
        ("size", json!(9007199254740992u64)),
        ("bitRate", json!(9007199254740992u64)),
        ("duration", json!(-1)),
        ("duration", json!(1e30)),
        ("year", json!(2000.5)),
        ("year", json!(2147483648u64)),
        ("artists", json!({})),
        ("artists", json!([{"name":"A"}])),
        ("genres", json!([null])),
        ("contributors", json!([{"role":"composer"}])),
        ("starred", json!(1)),
        ("replayGain", json!({"albumGain":"bad"})),
        ("releaseDate", json!({"year":"2020"})),
        ("track", json!(1.2)),
        ("playCount", json!(-2)),
    ] {
        let mut song = minimal();
        song[key] = value;
        let f = fake(song);
        assert_eq!(
            read(&f, &s, context(&s), &identity("song /1")),
            Err(ExchangeError::Malformed),
            "{key}"
        );
    }
    for key in ["id", "title", "contentType", "created", "size"] {
        let mut song = minimal();
        song.as_object_mut().unwrap().remove(key);
        assert_eq!(
            read(&fake(song), &s, context(&s), &identity("song /1")),
            Err(ExchangeError::Malformed)
        );
    }
    for body in [
        b"not json".to_vec(),
        br#"{"subsonic-response":{"status":"ok","version":"1"}}"#.to_vec(),
    ] {
        let mut f = fake(minimal());
        f.body = body;
        assert_eq!(
            read(&f, &s, context(&s), &identity("song /1")),
            Err(ExchangeError::Malformed)
        );
    }
}
#[test]
fn numeric_zero_ids_follow_javascript_to_string() {
    for n in [json!(0), serde_json::from_str("-0.0").unwrap()] {
        let s = state();
        let mut song = minimal();
        song["id"] = n.clone();
        song["coverArt"] = n;
        let result = read(&fake(song), &s, context(&s), &identity("0")).unwrap();
        assert_eq!(result.id.as_str(), "0");
        assert_eq!(result.image_id.as_deref(), Some("0"));
    }
}
#[test]
fn operation_guards_and_error_categories() {
    let s = state();
    let id = identity("song /1");
    for (status, expected) in [
        (401, HttpError::Authentication),
        (403, HttpError::Forbidden),
        (404, HttpError::NotFound),
        (400, HttpError::Client),
        (500, HttpError::Server),
        (201, HttpError::Other),
    ] {
        let mut f = fake(minimal());
        f.status = status;
        assert_eq!(
            read(&f, &s, context(&s), &id),
            Err(ExchangeError::Http(expected))
        );
    }
    for code in [0, 10, 20, 30, 40, 50, 60, 70, 999] {
        let mut f = fake(minimal());
        f.body=serde_json::to_vec(&json!({"subsonic-response":{"status":"failed","version":"1","error":{"code":code,"message":"synthetic-secret"}}})).unwrap();
        let e = read(&f, &s, context(&s), &id).unwrap_err();
        assert!(matches!(e, ExchangeError::Protocol(_)));
        assert!(!format!("{e:?}").contains("synthetic-secret"));
    }
    for error in [
        TransportError::Network,
        TransportError::Timeout,
        TransportError::Cancelled,
        TransportError::Other,
    ] {
        let mut f = fake(minimal());
        f.error = Some(error);
        assert_eq!(
            read(&f, &s, context(&s), &id),
            Err(ExchangeError::Transport(error))
        );
    }
    let mut f = fake(minimal());
    f.stale = true;
    assert_eq!(read(&f, &s, context(&s), &id), Err(ExchangeError::Stale));
    for dimension in 0..3 {
        let f = fake(minimal());
        let mut c = context(&s);
        match dimension {
            0 => c.limits.items = 0,
            1 => c.limits.request_bytes = 0,
            _ => c.limits.response_bytes = 0,
        };
        assert_eq!(
            read(&f, &s, c, &id),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
        if dimension < 2 {
            assert!(f.urls.borrow().is_empty())
        }
    }
    for key in ["artists", "albumArtists", "contributors", "genres"] {
        let mut song = minimal();
        song[key] = json!([null, null]);
        let f = fake(song);
        let mut c = context(&s);
        c.limits.items = 1;
        assert_eq!(
            read(&f, &s, c, &id),
            Err(ExchangeError::Context(DomainError::Capacity))
        );
    }
    let f = fake(minimal());
    s.cancelled.set(true);
    assert_eq!(
        read(&f, &s, context(&s), &id),
        Err(ExchangeError::Context(DomainError::Cancelled))
    );
    assert!(f.urls.borrow().is_empty());
    s.cancelled.set(false);
    s.now.set(11);
    assert_eq!(
        read(&f, &s, context(&s), &id),
        Err(ExchangeError::Context(DomainError::Deadline))
    );
    assert!(f.urls.borrow().is_empty());
}
struct PendingTransport;
impl ProtocolTransport for PendingTransport {
    fn exchange<'a>(
        &'a self,
        _: PlannedRequest,
        _: OperationContext<'a>,
    ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
        Box::pin(std::future::pending())
    }
}
#[test]
fn pending_non_send_future_rechecks_cancel_and_deadline() {
    for cancel in [false, true] {
        let s = state();
        let base = ServerBase::parse("http://127.0.0.1").unwrap();
        let auth = Auth::legacy("dummy", "synthetic").unwrap();
        let id = identity("song /1");
        let mut f = Box::pin(get_song(
            &PendingTransport,
            &s,
            context(&s),
            &base,
            &auth,
            &id,
        ));
        let w = Waker::from(Arc::new(Noop));
        let mut cx = Context::from_waker(&w);
        assert!(f.as_mut().poll(&mut cx).is_pending());
        if cancel {
            s.cancelled.set(true)
        } else {
            s.now.set(11)
        }
        assert_eq!(
            f.as_mut().poll(&mut cx),
            Poll::Ready(Err(ExchangeError::Context(if cancel {
                DomainError::Cancelled
            } else {
                DomainError::Deadline
            })))
        );
    }
}

#[test]
fn merged_artist_output_is_bounded_and_late_completion_is_rejected() {
    let s = state();
    let mut input = minimal();
    input["artists"] = json!([{"id":"a","name":"A"}]);
    input["contributors"] = json!([{"role":"remixer","artist":{"id":"r","name":"R"}}]);
    let f = fake(input);
    let mut c = context(&s);
    c.limits.items = 1;
    assert_eq!(
        read(&f, &s, c, &identity("song /1")),
        Err(ExchangeError::Context(DomainError::Capacity))
    );

    struct Late<'a> {
        state: &'a State,
        cancel: bool,
    }
    impl ProtocolTransport for Late<'_> {
        fn exchange<'a>(
            &'a self,
            _: PlannedRequest,
            ctx: OperationContext<'a>,
        ) -> LocalFuture<'a, Result<TransportResponse, TransportError>> {
            Box::pin(async move {
                if self.cancel {
                    self.state.cancelled.set(true)
                } else {
                    self.state.now.set(11)
                }
                Ok(TransportResponse {
                    total_count_header: None,
                    correlation: ctx.correlation,
                    status: 200,
                    body: fake(minimal()).body,
                })
            })
        }
    }
    for cancel in [false, true] {
        let s = state();
        let base = ServerBase::parse("https://example.test").unwrap();
        let auth = Auth::legacy("dummy", "synthetic").unwrap();
        let result = run(get_song(
            &Late { state: &s, cancel },
            &s,
            context(&s),
            &base,
            &auth,
            &identity("song /1"),
        ));
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

#[test]
fn negative_zero_release_components_match_source_padding() {
    let s = state();
    let mut input = minimal();
    input["releaseDate"] =
        serde_json::from_str(r#"{"year":2020,"month":-0.0,"day":-0.0}"#).unwrap();
    let song = read(&fake(input), &s, context(&s), &identity("song /1")).unwrap();
    assert_eq!(song.release_date.as_deref(), Some("2020-00-00"));
    assert_eq!(song.release_year, Some(2020));
}

#[test]
fn profile_server_identity_is_validated_before_dispatch_without_capping_song_id() {
    let s = state();
    for server in [
        String::new(),
        " \t\r\n".into(),
        "\u{2003}".into(),
        "a".repeat(129),
        "é".repeat(65),
    ] {
        let f = fake(minimal());
        let id = Scoped {
            server: ServerId::new(server),
            id: SongId::new("song /1"),
        };
        assert_eq!(
            read(&f, &s, context(&s), &id),
            Err(ExchangeError::Malformed)
        );
        assert!(f.urls.borrow().is_empty());
    }
    // This identity is exactly 128 UTF-8 bytes. Source song IDs use the existing
    // bounded query construction, so a 129-byte song ID must remain accepted.
    let server = "é".repeat(64);
    assert_eq!(server.len(), 128);
    let long_song_id = "s".repeat(129);
    let mut input = minimal();
    input["id"] = json!(long_song_id);
    let f = fake(input);
    let id = Scoped {
        server: ServerId::new(server),
        id: SongId::new(long_song_id),
    };
    let song = read(&f, &s, context(&s), &id).unwrap();
    assert_eq!(song.server, id.server);
    assert_eq!(song.id, id.id);
    assert_eq!(f.urls.borrow().len(), 1);
}

#[test]
fn completion_context_takes_priority_over_normalization_and_identity_errors() {
    struct NormalizationClock<'a> {
        calls: Cell<usize>,
        state: &'a State,
        cancel: bool,
    }
    impl Clock for NormalizationClock<'_> {
        fn now(&self) -> Milliseconds {
            let calls = self.calls.get() + 1;
            self.calls.set(calls);
            // The first five checks cover get_song entry, exchange entry, the
            // transport poll/completion and envelope parsing. Only the final
            // song normalization completion check observes this state change.
            if calls > 5 {
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
        for kind in 0..4 {
            let s = state();
            let clock = NormalizationClock {
                calls: Cell::new(0),
                state: &s,
                cancel,
            };
            let mut input = minimal();
            match kind {
                0 => {
                    input.as_object_mut().unwrap().remove("size");
                }
                1 => {
                    input = Value::Null;
                }
                2 => {
                    input["id"] = json!("wrong-id");
                }
                _ => {}
            }
            let f = fake(input);
            let result = run(get_song(
                &f,
                &clock,
                context(&s),
                &ServerBase::parse("https://example.test").unwrap(),
                &Auth::legacy("dummy", "synthetic").unwrap(),
                &identity("song /1"),
            ));
            assert_eq!(
                result,
                Err(ExchangeError::Context(if cancel {
                    DomainError::Cancelled
                } else {
                    DomainError::Deadline
                }))
            );
            assert_eq!(f.urls.borrow().len(), 1);
        }
    }
}
