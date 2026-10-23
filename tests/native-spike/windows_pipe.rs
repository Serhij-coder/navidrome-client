#![cfg(windows)]
use feishin_native_spike::{AudioOutput, Error, Mpv, write_tone};
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::Path,
    time::{Duration, Instant},
};

fn peer(mode: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let fixture = Path::new(env!("CARGO_BIN_EXE_p01-pipe-peer"));
    // Same volume as the built fixture: hard links avoid newly written executable
    // files and do not require symbolic-link/developer-mode permission on Windows.
    let directory = tempfile::tempdir_in(fixture.parent().unwrap()).unwrap();
    let binary = directory.path().join(format!("{mode}.exe"));
    std::fs::hard_link(fixture, &binary).unwrap();
    (directory, binary)
}

#[test]
fn windows_pipe_correlates_replies_and_preserves_input_and_generation_contracts() {
    let (directory, binary) = peer("normal");
    let tone = directory.path().join("owned tone.wav");
    write_tone(&tone, 1, 750).unwrap();
    let mut mpv = Mpv::start(&binary, 17, AudioOutput::Null).unwrap();
    let ipc_directory = mpv.socket_directory().to_owned();
    assert!(mpv.accepts_generation(17));
    assert!(!mpv.accepts_generation(16));
    assert_eq!(
        mpv.probe_position(Duration::from_secs(1)).unwrap(),
        Some(0.0)
    );
    mpv.load_local(&tone, false).unwrap();
    mpv.load_local(&tone, true).unwrap();
    assert_eq!(mpv.playlist_count().unwrap(), 2);
    let invalid = directory.path().join(OsString::from_wide(&[0xD800]));
    assert!(matches!(
        mpv.load_local(&invalid, false),
        Err(Error::InvalidInput)
    ));
    assert!(matches!(mpv.seek(f64::NAN), Err(Error::InvalidInput)));
    assert_eq!(mpv.playlist_count().unwrap(), 2);
    mpv.pause(true).unwrap();
    mpv.seek(1.0).unwrap();
    mpv.pause(false).unwrap();
    mpv.low_pass_probe().unwrap();
    mpv.shutdown().unwrap();
    assert!(!mpv.accepts_generation(17));
    mpv.shutdown().unwrap();
    drop(mpv);
    assert!(!ipc_directory.exists());
}

#[test]
fn windows_pipe_malformed_oversized_and_dead_peers_are_controlled_errors() {
    for mode in ["malformed", "oversized", "die"] {
        let (_directory, binary) = peer(mode);
        let mut mpv = Mpv::start(&binary, 1, AudioOutput::Null).unwrap();
        let result = mpv.playlist_count();
        if mode != "die" {
            assert!(matches!(result, Err(Error::Protocol)), "{mode}: {result:?}");
        } else {
            assert!(result.is_err());
        }
        assert!(!mpv.accepts_generation(1));
        mpv.shutdown().unwrap();
    }
}

#[test]
fn windows_pipe_partial_reply_deadlines_and_stalled_quit_are_bounded() {
    for mode in ["missing-pipe", "slow-startup"] {
        let (_directory, binary) = peer(mode);
        let start = Instant::now();
        assert!(matches!(
            Mpv::start(&binary, 1, AudioOutput::Null),
            Err(Error::Deadline)
        ));
        assert!(start.elapsed() < Duration::from_secs(6));
    }
    let (_directory, binary) = peer("slow-command");
    let mut mpv = Mpv::start(&binary, 1, AudioOutput::Null).unwrap();
    let start = Instant::now();
    assert!(matches!(mpv.playlist_count(), Err(Error::Deadline)));
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(!mpv.accepts_generation(1));
    mpv.shutdown().unwrap();
    let (_directory, binary) = peer("stall-quit");
    let mut mpv = Mpv::start(&binary, 2, AudioOutput::Null).unwrap();
    let start = Instant::now();
    mpv.shutdown().unwrap();
    drop(mpv);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn windows_position_queries_distinguish_unavailable_invalid_and_rejected() {
    let (_directory, binary) = peer("unavailable-position");
    let mut player = Mpv::start(&binary, 32, AudioOutput::Null).unwrap();
    assert_eq!(player.probe_position(Duration::from_secs(1)).unwrap(), None);
    assert!(matches!(player.position(), Err(Error::PropertyUnavailable)));
    assert!(player.accepts_generation(32));
    player.pause(true).unwrap();
    player.shutdown().unwrap();
    for mode in ["rejected-position", "negative-position", "null-position"] {
        let (_directory, binary) = peer(mode);
        let mut player = Mpv::start(&binary, 33, AudioOutput::Null).unwrap();
        let result = player.probe_position(Duration::from_secs(1));
        if mode == "rejected-position" {
            assert!(matches!(result, Err(Error::Rejected)));
        } else {
            assert!(matches!(result, Err(Error::Protocol)));
        }
        assert!(!player.accepts_generation(33));
        player.shutdown().unwrap();
    }
}
