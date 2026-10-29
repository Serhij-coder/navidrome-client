use feishin_subsonic::{Auth, Entropy, EntropyError, Error, ServerBase};

struct Bytes(u8);
impl Entropy for Bytes {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), EntropyError> {
        bytes[0] = self.0;
        self.0 = self.0.wrapping_add(1);
        Ok(())
    }
}
struct Reject;
impl Entropy for Reject {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), EntropyError> {
        bytes[0] = 255;
        Ok(())
    }
}
struct Fail;
impl Entropy for Fail {
    fn fill(&mut self, _: &mut [u8]) -> Result<(), EntropyError> {
        Err(EntropyError)
    }
}
#[test]
fn salted_wire_and_reuse() {
    let auth = Auth::salted("a+&= %!'()*?#/:名", "päss", &mut Bytes(0)).unwrap();
    let base = ServerBase::parse(" https://[::1]:8080/music/%E2%98%83/ ").unwrap();
    let request = base
        .request(
            &auth,
            "getSong.view",
            &[("id", "a b"), ("id", ""), ("query", "null")],
        )
        .unwrap();
    assert_eq!(
        request.expose_for_transport(),
        "https://[::1]:8080/music/%E2%98%83/rest/getSong.view?c=Feishin&f=json&v=1.13.0&u=a%2B%26%3D%20%25%21%27%28%29%2A%3F%23%2F%3A%E5%90%8D&s=ABCDEFGHIJKL&t=03ac761caad465f927e670fa65f5aac0&id=a%20b&id=&query=null"
    );
    assert_eq!(
        base.request(&auth, "getSong.view", &[])
            .unwrap()
            .expose_for_transport()
            .split('&')
            .find(|p| p.starts_with("s=")),
        Some("s=ABCDEFGHIJKL")
    );
    assert_eq!(
        format!("{auth:?} {request:?} {base:?}"),
        "Auth([REDACTED]) RequestUrl([REDACTED]) ServerBase([REDACTED])"
    );
}
#[test]
fn legacy_is_explicit_and_escapes() {
    let auth = Auth::legacy("", "+&= %!'()*?#/:é").unwrap();
    let base = ServerBase::parse("http://192.168.1.1/rest//").unwrap();
    assert_eq!(
        base.request(&auth, "ping", &[])
            .unwrap()
            .expose_for_transport(),
        "http://192.168.1.1/rest//rest/ping?c=Feishin&f=json&v=1.13.0&u=&p=%2B%26%3D%20%25%21%27%28%29%2A%3F%23%2F%3A%C3%A9"
    );
}
#[test]
fn invalid_inputs_are_closed_and_redacted() {
    for input in [
        "file:///secret",
        "http://user:secret@host",
        "http://@host",
        "http://host/%xx",
        "http://host/?secret",
        "http://host/#secret",
        "http://host/\nsecret",
        "http://host\\secret",
    ] {
        let err = ServerBase::parse(input).unwrap_err();
        assert_eq!(err, Error::InvalidBase);
        assert!(!format!("{err:?} {err}").contains("secret"));
    }
    let base = ServerBase::parse("http://host").unwrap();
    let auth = Auth::legacy("user", "secret").unwrap();
    for endpoint in [
        "", ".", "..", "../x", "%2e%2e", "x%2fy", "x?y", "x#y", "x\\y",
    ] {
        assert_eq!(
            base.request(&auth, endpoint, &[]).unwrap_err(),
            Error::InvalidEndpoint
        );
    }
    for key in [
        "c",
        "U",
        "password",
        "authorization",
        "token",
        "u[]",
        "c[x]",
        "",
    ] {
        assert_eq!(
            base.request(&auth, "ping", &[(key, "secret")]).unwrap_err(),
            Error::InvalidQuery
        );
    }
    assert_eq!(
        base.request(&auth, "ping", &vec![("x", "y"); 129])
            .unwrap_err(),
        Error::InputTooLarge
    );
    assert_eq!(
        base.request(&auth, "ping", &[("x", &"x".repeat(8193))])
            .unwrap_err(),
        Error::InputTooLarge
    );
}
#[test]
fn entropy_failure_and_rejection_are_bounded() {
    assert_eq!(
        Auth::salted("u", "p", &mut Reject).unwrap_err(),
        Error::EntropyUnavailable
    );
    assert_eq!(
        Auth::salted("u", "p", &mut Fail).unwrap_err(),
        Error::EntropyUnavailable
    );
    let auth = Auth::salted("u", "p", &mut Bytes(247)).unwrap();
    let url = ServerBase::parse("http://host")
        .unwrap()
        .request(&auth, "ping", &[])
        .unwrap();
    assert!(url.expose_for_transport().contains("&s=9ABCDEFGHIJK&"));
}

#[test]
fn input_limits_and_non_send_entropy() {
    use std::{cell::Cell, rc::Rc};
    struct Count(Rc<Cell<usize>>);
    impl Entropy for Count {
        fn fill(&mut self, bytes: &mut [u8]) -> Result<(), EntropyError> {
            let n = self.0.get();
            self.0.set(n + 1);
            bytes.fill(186 + n as u8);
            Ok(())
        }
    }
    let count = Rc::new(Cell::new(0));
    let mut entropy = Count(count.clone());
    assert_eq!(
        Auth::salted(&"x".repeat(8193), "p", &mut entropy).unwrap_err(),
        Error::InputTooLarge
    );
    assert_eq!(
        Auth::salted("u", &"x".repeat(8193), &mut entropy).unwrap_err(),
        Error::InputTooLarge
    );
    assert_eq!(count.get(), 0);
    assert_eq!(
        ServerBase::parse(&"x".repeat(8193)).unwrap_err(),
        Error::InputTooLarge
    );
    let auth = Auth::salted("u", "p", &mut entropy).unwrap();
    assert_eq!(count.get(), 12);
    let base = ServerBase::parse("http://host").unwrap();
    let request = base.request(&auth, "ping", &[]).unwrap();
    assert!(request.expose_for_transport().contains("&s=ABCDEFGHIJKL&"));
    let value = "é".repeat(4096);
    let pairs = vec![("id", value.as_str()); 3];
    assert_eq!(
        base.request(&auth, "ping", &pairs).unwrap_err(),
        Error::InputTooLarge
    );
}

#[test]
fn query_encoding_and_literal_username_percent() {
    let base = ServerBase::parse("http://host").unwrap();
    for (username, expected) in [("percent%name", "percent%25name"), ("%2F", "%252F")] {
        let auth = Auth::legacy(username, "password").unwrap();
        let request = base
            .request(&auth, "ping", &[("名+&", "+/%"), ("名+&", "")])
            .unwrap();
        let wire = request.expose_for_transport();
        assert!(wire.contains(&format!("&u={expected}&")));
        assert!(wire.ends_with("&%E5%90%8D%2B%26=%2B%2F%25&%E5%90%8D%2B%26="));
        let redacted = format!("{auth:?} {request:?}");
        for sensitive in [username, expected, "password", "%E5%90%8D"] {
            assert!(!redacted.contains(sensitive));
        }
    }
    for error in [
        Error::InvalidBase,
        Error::InvalidEndpoint,
        Error::InvalidQuery,
        Error::InputTooLarge,
        Error::EntropyUnavailable,
    ] {
        let text = format!("{error:?} {error}");
        for sensitive in ["secret", "percent%25name", "password"] {
            assert!(!text.contains(sensitive));
        }
    }
}

#[test]
fn source_replay_wire_vectors() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p03/subsonic-foundation.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let username = case["username"].as_str().unwrap();
        let password = case["password"].as_str().unwrap();
        let auth = if case["legacy"].as_bool().unwrap() {
            Auth::legacy(username, password).unwrap()
        } else {
            Auth::salted(username, password, &mut Bytes(0)).unwrap()
        };
        let pairs: Vec<_> = case["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| (pair[0].as_str().unwrap(), pair[1].as_str().unwrap()))
            .collect();
        let base = ServerBase::parse(case["base"].as_str().unwrap()).unwrap();
        let url = base.request(&auth, "fixture.view", &pairs).unwrap();
        let wire = url.expose_for_transport();
        if id == "literal-percent-username" {
            assert_eq!(case["expected"]["request_error"], "URIError");
            assert!(wire.contains("&u=percent%25name&"));
            let token = case["expected"]["token"].as_str().unwrap();
            assert!(wire.contains(&format!("&t={token}")));
        } else {
            assert_eq!(
                wire,
                case["expected"]["wire_url"].as_str().unwrap(),
                "fixture {id}"
            );
        }
    }
}
