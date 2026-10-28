use super::*;

#[test]
fn explicit_loopback_fixture_authority_preserves_opaque_query_data() {
    for source in [
        "http://127.0.0.1:49152/owned.wav?token=fixture-only",
        "http://127.0.0.1:80/owned.flac?token=opaque%26value",
        "http://127.0.0.1:65535/owned.mp3",
    ] {
        assert!(validate_loopback_probe_url(source).is_ok());
    }
}

#[test]
fn rejects_ambiguous_external_authorities_credentials_controls_and_unbounded_input() {
    for source in [
        "",
        "file:///tmp/owned.wav",
        "https://127.0.0.1:1234/a",
        "http://127.0.0.1/a",
        "http://127.0.0.1:0/a",
        "http://127.0.0.1:65536/a",
        "http://127.0.0.1:+12/a",
        "http://127.0.0.1:1234@external.invalid/a",
        "http://user:secret@127.0.0.1:1234/a",
        "http://localhost:1234/a",
        "http://127.1:1234/a",
        "http://[::1]:1234/a",
        "http://0.0.0.0:1234/a",
        "http://127.0.0.1:1234/a#fragment",
        " http://127.0.0.1:1234/a",
        "http://127.0.0.1:1234/a\r\nInjected: value",
        "http://127.0.0.1:1234/a\0",
        "http://127.0.0.1:1234/with space",
    ] {
        assert!(matches!(
            validate_loopback_probe_url(source),
            Err(Error::InvalidInput)
        ));
    }
    let large = format!("http://127.0.0.1:1234/{}", "a".repeat(4096));
    assert!(matches!(
        validate_loopback_probe_url(&large),
        Err(Error::InvalidInput)
    ));
}
