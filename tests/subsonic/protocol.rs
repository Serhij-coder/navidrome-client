use feishin_domain::{
    Cancellation, Clock, Generation, Milliseconds, OperationContext, OperationLimits,
    RequestContext, RequestId,
};
use feishin_subsonic::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
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
struct Fake {
    replies: RefCell<VecDeque<(u16, Vec<u8>, bool)>>,
    urls: RefCell<Vec<String>>,
}
impl ProtocolTransport for Fake {
    fn exchange<'a>(
        &'a self,
        req: PlannedRequest,
        ctx: OperationContext<'a>,
    ) -> feishin_domain::LocalFuture<'a, Result<TransportResponse, TransportError>> {
        self.urls
            .borrow_mut()
            .push(req.expose_url_for_transport().to_owned());
        let (status, body, stale) = self.replies.borrow_mut().pop_front().unwrap();
        Box::pin(async move {
            let mut correlation = ctx.correlation;
            if stale {
                correlation.request = RequestId(2);
            }
            Ok(TransportResponse {
                correlation,
                status,
                body,
            })
        })
    }
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
fn run<F: Future>(f: F) -> F::Output {
    let mut f = Box::pin(f);
    let w = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&w);
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("unexpected pending"),
    }
}
fn context(s: &State) -> OperationContext<'_> {
    OperationContext {
        correlation: RequestContext {
            request: RequestId(1),
            generation: Generation(1),
            deadline: Milliseconds(10),
        },
        limits: OperationLimits {
            request_bytes: 10000,
            response_bytes: 10000,
            items: 20,
        },
        cancellation: s,
    }
}
fn fake(replies: &[&str]) -> Fake {
    Fake {
        replies: RefCell::new(
            replies
                .iter()
                .map(|s| (200, s.as_bytes().to_vec(), false))
                .collect(),
        ),
        urls: RefCell::new(vec![]),
    }
}
const PING: &str = r#"{"subsonic-response":{"status":"ok","version":"1.16.1","openSubsonic":true,"serverVersion":"2"}}"#;
const EXT: &str = r#"{"subsonic-response":{"status":"ok","version":"1.16.1","openSubsonicExtensions":[{"name":"formPost","versions":[2]},{"name":"formPost","versions":[1]},{"name":"unknown","versions":[1]}]}}"#;
#[test]
fn discovery_and_form_post() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(10),
    };
    let f = fake(&[PING, EXT]);
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("a@b", "secret").unwrap();
    let d = run(discover(&f, &s, context(&s), &base, &auth)).unwrap();
    assert!(d.supports("formPost", 1));
    assert!(!d.supports("formPost", 2));
    assert_eq!(d.versions("unknown"), None);
    let p = plan_request(
        &base,
        &auth,
        "ping.view",
        &[("x", "a b"), ("x", "c")],
        PostPreference::RequireForm,
        Some(&d),
    )
    .unwrap();
    assert!(!p.expose_url_for_transport().contains('?'));
    assert!(p.expose_body_for_transport().unwrap().contains("u=a%40b"));
    assert!(
        p.expose_body_for_transport()
            .unwrap()
            .contains("x=a%20b&x=c")
    );
    assert!(!format!("{p:?}").contains("secret"));
    assert_eq!(f.urls.borrow().len(), 2);
}
#[test]
fn failed_envelopes_and_redaction() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    for (code, expected) in [
        ("0", ProtocolError::Generic),
        ("\"40\"", ProtocolError::Authentication),
    ] {
        let body = format!(
            r#"{{"subsonic-response":{{"status":"failed","version":"1","error":{{"code":{code},"message":"secret"}}}}}}"#
        );
        let f = fake(&[&body]);
        let e = run(discover(&f, &s, context(&s), &base, &auth)).unwrap_err();
        assert_eq!(e, ExchangeError::Protocol(expected));
        assert!(!format!("{e:?}").contains("secret"));
    }
}
#[test]
fn limits_http_stale_and_cancellation() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    let f = fake(&[PING]);
    let mut c = context(&s);
    c.limits.response_bytes = 1;
    assert_eq!(
        run(discover(&f, &s, c, &base, &auth)),
        Err(ExchangeError::Context(
            feishin_domain::DomainError::Capacity
        ))
    );
    let f = fake(&[PING]);
    f.replies.borrow_mut()[0].2 = true;
    assert_eq!(
        run(discover(&f, &s, context(&s), &base, &auth)),
        Err(ExchangeError::Stale)
    );
    let f = fake(&[PING]);
    f.replies.borrow_mut()[0].0 = 403;
    assert_eq!(
        run(discover(&f, &s, context(&s), &base, &auth)),
        Err(ExchangeError::Http(HttpError::Forbidden))
    );
    s.cancelled.set(true);
    let f = fake(&[]);
    assert_eq!(
        run(discover(&f, &s, context(&s), &base, &auth)),
        Err(ExchangeError::Context(
            feishin_domain::DomainError::Cancelled
        ))
    );
}
#[test]
fn malformed_and_unsupported() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    assert!(matches!(
        plan_request(
            &base,
            &auth,
            "ping.view",
            &[],
            PostPreference::RequireForm,
            None
        ),
        Err(ExchangeError::Unsupported)
    ));
    let f = fake(&[
        PING,
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonicExtensions":[{"name":"formPost","versions":[0]}]}}"#,
    ]);
    assert_eq!(
        run(discover(&f, &s, context(&s), &base, &auth)),
        Err(ExchangeError::Malformed)
    );
}

struct PendingTransport {
    state: std::rc::Rc<State>,
    complete: bool,
}
impl ProtocolTransport for PendingTransport {
    fn exchange<'a>(
        &'a self,
        _: PlannedRequest,
        context: OperationContext<'a>,
    ) -> feishin_domain::LocalFuture<'a, Result<TransportResponse, TransportError>> {
        let state = self.state.clone();
        let complete = self.complete;
        Box::pin(std::future::poll_fn(move |_| {
            if complete {
                state.cancelled.set(true);
                Poll::Ready(Ok(TransportResponse {
                    correlation: context.correlation,
                    status: 200,
                    body: PING.as_bytes().to_vec(),
                }))
            } else {
                Poll::Pending
            }
        }))
    }
}
#[test]
fn cooperative_pending_and_post_completion_checks() {
    let state = std::rc::Rc::new(State {
        cancelled: Cell::new(false),
        now: Cell::new(10),
    });
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    let transport = PendingTransport {
        state: state.clone(),
        complete: false,
    };
    let mut future = Box::pin(discover(&transport, &*state, context(&state), &base, &auth));
    let w = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&w);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    state.now.set(11);
    assert_eq!(
        future.as_mut().poll(&mut cx),
        Poll::Ready(Err(ExchangeError::Context(
            feishin_domain::DomainError::Deadline
        )))
    );
    state.now.set(10);
    let transport = PendingTransport {
        state: state.clone(),
        complete: true,
    };
    assert_eq!(
        run(discover(&transport, &*state, context(&state), &base, &auth)),
        Err(ExchangeError::Context(
            feishin_domain::DomainError::Cancelled
        ))
    );
}

#[test]
fn discovery_binding_and_transport_metadata() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let a = ServerBase::parse("https://example.com/").unwrap();
    let same = ServerBase::parse("https://example.com").unwrap();
    let b = ServerBase::parse("https://other.example").unwrap();
    let auth = Auth::legacy("u", "secret").unwrap();
    let f = fake(&[PING, EXT]);
    let d = run(discover(&f, &s, context(&s), &a, &auth)).unwrap();
    let post = plan_request(
        &same,
        &auth,
        "ping.view",
        &[],
        PostPreference::RequireForm,
        Some(&d),
    )
    .unwrap();
    assert_eq!(
        post.content_type(),
        Some("application/x-www-form-urlencoded")
    );
    assert!(matches!(
        plan_request(
            &b,
            &auth,
            "ping.view",
            &[],
            PostPreference::RequireForm,
            Some(&d)
        ),
        Err(ExchangeError::Unsupported)
    ));
    let get = plan_request(
        &b,
        &auth,
        "ping.view",
        &[],
        PostPreference::PreferForm,
        Some(&d),
    )
    .unwrap();
    assert_eq!(get.content_type(), None);
    assert!(get.expose_body_for_transport().is_none());
    let f = fake(&[
        r#"{"subsonic-response":{"status":"ok","version":"secret","serverVersion":"secret"}}"#,
    ]);
    let d = run(discover(&f, &s, context(&s), &a, &auth)).unwrap();
    assert!(!format!("{d:?}").contains("secret"));
}

struct GenerationMismatch;
impl ProtocolTransport for GenerationMismatch {
    fn exchange<'a>(
        &'a self,
        _: PlannedRequest,
        context: OperationContext<'a>,
    ) -> feishin_domain::LocalFuture<'a, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            let mut correlation = context.correlation;
            correlation.generation = Generation(2);
            Ok(TransportResponse {
                correlation,
                status: 200,
                body: PING.as_bytes().to_vec(),
            })
        })
    }
}
#[test]
fn request_and_item_limits_and_generation() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    let f = fake(&[]);
    let mut c = context(&s);
    c.limits.request_bytes = 1;
    assert_eq!(
        run(discover(&f, &s, c, &base, &auth)),
        Err(ExchangeError::Context(
            feishin_domain::DomainError::Capacity
        ))
    );
    assert!(f.urls.borrow().is_empty());
    assert_eq!(
        run(discover(&GenerationMismatch, &s, context(&s), &base, &auth)),
        Err(ExchangeError::Stale)
    );
    for ext in [
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonicExtensions":[{"name":"formPost","versions":[1]},{"name":"songLyrics","versions":[1]}]}}"#,
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonicExtensions":[{"name":"formPost","versions":[1,2]}]}}"#,
    ] {
        let f = fake(&[PING, ext]);
        let mut c = context(&s);
        c.limits.items = 1;
        assert_eq!(
            run(discover(&f, &s, c, &base, &auth)),
            Err(ExchangeError::Context(
                feishin_domain::DomainError::Capacity
            ))
        );
    }
}
#[test]
fn malformed_optional_and_error_mappings() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    for body in [
        "{",
        "{}",
        "[]",
        r#"{"subsonic-response":[]}"#,
        r#"{"subsonic-response":{"status":"ok"}}"#,
        r#"{"subsonic-response":{"status":"other","version":"1"}}"#,
        r#"{"subsonic-response":{"status":"failed","version":"1","error":{"code":"oops"}}}"#,
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonic":"true"}}"#,
        r#"{"subsonic-response":{"status":"ok","version":"1","serverVersion":1}}"#,
    ] {
        let f = fake(&[body]);
        assert_eq!(
            run(discover(&f, &s, context(&s), &base, &auth)),
            Err(ExchangeError::Malformed)
        );
    }
    for body in [
        r#"{"subsonic-response":{"status":"ok","version":"1"}}"#,
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonic":true,"serverVersion":""}}"#,
    ] {
        let f = fake(&[body]);
        assert!(run(discover(&f, &s, context(&s), &base, &auth)).is_ok());
        assert_eq!(f.urls.borrow().len(), 1);
    }
    let f = fake(&[
        PING,
        r#"{"subsonic-response":{"status":"ok","version":"1"}}"#,
    ]);
    assert!(
        run(discover(&f, &s, context(&s), &base, &auth))
            .unwrap()
            .versions("formPost")
            .is_none()
    );
    for body in [
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonicExtensions":{}}}"#,
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonicExtensions":[{"name":"","versions":[]}]}}"#,
        r#"{"subsonic-response":{"status":"ok","version":"1","openSubsonicExtensions":[{"name":"formPost","versions":["1"]}]}}"#,
    ] {
        let f = fake(&[PING, body]);
        assert_eq!(
            run(discover(&f, &s, context(&s), &base, &auth)),
            Err(ExchangeError::Malformed)
        );
    }
    for (code, expected) in [
        (10, ProtocolError::MissingParameter),
        (20, ProtocolError::IncompatibleClient),
        (30, ProtocolError::IncompatibleServer),
        (40, ProtocolError::Authentication),
        (50, ProtocolError::Forbidden),
        (60, ProtocolError::TrialExpired),
        (70, ProtocolError::DataNotFound),
        (-1, ProtocolError::Other),
    ] {
        let body = format!(
            r#"{{"subsonic-response":{{"status":"failed","version":"1","error":{{"code":{code}}}}}}}"#
        );
        let f = fake(&[&body]);
        assert_eq!(
            run(discover(&f, &s, context(&s), &base, &auth)),
            Err(ExchangeError::Protocol(expected))
        );
    }
    for (status, expected) in [
        (401, HttpError::Authentication),
        (403, HttpError::Forbidden),
        (404, HttpError::NotFound),
        (429, HttpError::Client),
        (500, HttpError::Server),
        (302, HttpError::Other),
        (201, HttpError::Other),
        (204, HttpError::Other),
    ] {
        let f = fake(&[PING]);
        f.replies.borrow_mut()[0].0 = status;
        assert_eq!(
            run(discover(&f, &s, context(&s), &base, &auth)),
            Err(ExchangeError::Http(expected))
        );
    }
}
struct ScriptedClock(Cell<u32>);
impl Clock for ScriptedClock {
    fn now(&self) -> Milliseconds {
        let n = self.0.get();
        self.0.set(n + 1);
        Milliseconds(if n >= 3 { 11 } else { 10 })
    }
}
#[test]
fn deadline_checked_after_parsing() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let clock = ScriptedClock(Cell::new(0));
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    let f = fake(&[PING]);
    assert_eq!(
        run(discover(&f, &clock, context(&s), &base, &auth)),
        Err(ExchangeError::Context(
            feishin_domain::DomainError::Deadline
        ))
    );
}

#[test]
fn form_post_requires_version_one_and_get_remains_get() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    for versions in ["[]", "[2]", "[1]"] {
        let ext = format!(
            r#"{{"subsonic-response":{{"status":"ok","version":"1","openSubsonicExtensions":[{{"name":"formPost","versions":{versions}}}]}}}}"#
        );
        let f = fake(&[PING, &ext]);
        let d = run(discover(&f, &s, context(&s), &base, &auth)).unwrap();
        let get = plan_request(
            &base,
            &auth,
            "ping.view",
            &[],
            PostPreference::Get,
            Some(&d),
        )
        .unwrap();
        assert!(matches!(get.method(), Method::Get));
        assert_eq!(get.content_type(), None);
        if versions != "[1]" {
            assert!(matches!(
                plan_request(
                    &base,
                    &auth,
                    "ping.view",
                    &[],
                    PostPreference::RequireForm,
                    Some(&d)
                ),
                Err(ExchangeError::Unsupported)
            ));
            let preferred = plan_request(
                &base,
                &auth,
                "ping.view",
                &[],
                PostPreference::PreferForm,
                Some(&d),
            )
            .unwrap();
            assert!(matches!(preferred.method(), Method::Get));
            assert!(preferred.expose_body_for_transport().is_none());
        }
    }
}
struct FailedTransport;
impl ProtocolTransport for FailedTransport {
    fn exchange<'a>(
        &'a self,
        _: PlannedRequest,
        _: OperationContext<'a>,
    ) -> feishin_domain::LocalFuture<'a, Result<TransportResponse, TransportError>> {
        Box::pin(async { Err(TransportError::Network) })
    }
}
#[test]
fn transport_failure_and_response_debug_are_redacted() {
    let s = State {
        cancelled: Cell::new(false),
        now: Cell::new(0),
    };
    let base = ServerBase::parse("https://example.com").unwrap();
    let auth = Auth::legacy("u", "p").unwrap();
    assert_eq!(
        run(discover(&FailedTransport, &s, context(&s), &base, &auth)),
        Err(ExchangeError::Transport(TransportError::Network))
    );
    let response = TransportResponse {
        correlation: context(&s).correlation,
        status: 200,
        body: b"https://example.com/rest?u=user&p=secret".to_vec(),
    };
    let debug = format!("{response:?}");
    assert!(!debug.contains("secret"));
    assert!(!debug.contains("example.com"));
}
