use super::*;

/// Paused fixture helper, not a production seek coordinator or device timing test.
pub(super) fn wait_for_position(
    player: &mut Mpv,
    expected_seconds: f64,
    budget: Duration,
) -> Result<f64, Error> {
    let deadline = Instant::now() + budget;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or(Error::Deadline)?;
        let observed = player.probe_position(remaining)?;
        if Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        if let Some(position) = observed
            && (position - expected_seconds).abs() <= 0.1
        {
            return Ok(position);
        }
        std::thread::sleep(Duration::from_millis(5).min(remaining));
    }
}

#[test]
fn unavailable_position_is_nonfatal_and_seek_acknowledgement_can_precede_arrival() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("owned.wav");
    write_tone(&tone, 2, 750).unwrap();
    let mut player = Mpv::start(
        &fake(directory.path(), "delayed-position"),
        23,
        AudioOutput::Null,
    )
    .unwrap();
    assert_eq!(player.probe_position(Duration::from_secs(1)).unwrap(), None);
    assert!(matches!(player.position(), Err(Error::PropertyUnavailable)));
    assert!(player.accepts_generation(23));
    player.pause(true).unwrap();
    player.load_local(&tone, false).unwrap();
    wait_for_position(&mut player, 0.0, Duration::from_secs(1)).unwrap();
    player.seek(1.0).unwrap();
    // This reproduces the old test's invalid inference: accepted != arrived.
    assert_eq!(
        player.probe_position(Duration::from_secs(1)).unwrap(),
        Some(0.0)
    );
    assert_eq!(
        wait_for_position(&mut player, 1.0, Duration::from_secs(1)).unwrap(),
        1.0
    );
    assert!(player.accepts_generation(23));
}

#[test]
fn missing_readiness_and_unchanged_seek_position_exhaust_one_budget() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("owned.wav");
    write_tone(&tone, 2, 750).unwrap();
    for mode in ["unavailable-position", "never-seeks"] {
        let mut player = Mpv::start(&fake(directory.path(), mode), 24, AudioOutput::Null).unwrap();
        player.load_local(&tone, false).unwrap();
        player.seek(1.0).unwrap();
        let started = Instant::now();
        assert!(matches!(
            wait_for_position(&mut player, 1.0, Duration::from_millis(80)),
            Err(Error::Deadline)
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        // The caller may expire between queries or during one. An in-flight
        // query timeout invalidates framing; neither outcome may report success.
    }
}

#[test]
fn partial_position_reply_obeys_the_callers_shorter_deadline() {
    let directory = tempfile::tempdir().unwrap();
    let mut player = Mpv::start(
        &fake(directory.path(), "slow-position"),
        25,
        AudioOutput::Null,
    )
    .unwrap();
    let started = Instant::now();
    assert!(matches!(
        player.probe_position(Duration::from_millis(80)),
        Err(Error::Deadline)
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!player.accepts_generation(25));
    assert!(matches!(player.playlist_count(), Err(Error::ProcessExited)));
}

#[test]
fn invalid_position_values_and_unrelated_rejections_are_fatal() {
    let directory = tempfile::tempdir().unwrap();
    for mode in [
        "negative-position",
        "null-position",
        "string-position",
        "rejected-position",
    ] {
        let mut player = Mpv::start(&fake(directory.path(), mode), 26, AudioOutput::Null).unwrap();
        let result = player.probe_position(Duration::from_secs(1));
        if mode == "rejected-position" {
            assert!(matches!(result, Err(Error::Rejected)));
        } else {
            assert!(matches!(result, Err(Error::Protocol)));
        }
        assert!(!player.accepts_generation(26));
    }
}

#[test]
fn zero_position_budget_leaves_the_current_engine_usable() {
    let directory = tempfile::tempdir().unwrap();
    let mut player = Mpv::start(&fake(directory.path(), "normal"), 27, AudioOutput::Null).unwrap();
    assert!(matches!(
        player.probe_position(Duration::ZERO),
        Err(Error::Deadline)
    ));
    assert!(player.accepts_generation(27));
    assert_eq!(player.playlist_count().unwrap(), 0);
    assert_eq!(player.probe_position(Duration::MAX).unwrap(), Some(0.0));
}
