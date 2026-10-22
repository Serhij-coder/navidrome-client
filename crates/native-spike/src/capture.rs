use super::*;
use feishin_spike_model::{Analysis, PCM_SAMPLES};
use std::{
    io::Read,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Explicit Linux PulseAudio/PipeWire monitor consent. Never starts implicitly.
/// One newest analysis frame is retained; no growing PCM or UI-message queues.
pub struct Capture {
    child: ManagedChild,
    latest: Arc<Mutex<Option<Analysis>>>,
    stopped: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Capture {
    pub fn start_monitor(ffmpeg: &Path, source: &str) -> Result<Self, Error> {
        validate_monitor(source)?;
        Self::spawn(ffmpeg, &["-f", "pulse", "-i", source])
    }
    /// Deterministic owned tone source for lifecycle tests. It is not OS capture evidence.
    pub fn start_test_tone(ffmpeg: &Path) -> Result<Self, Error> {
        Self::spawn(
            ffmpeg,
            &[
                "-re",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=750:sample_rate=8000",
            ],
        )
    }
    fn spawn(ffmpeg: &Path, input: &[&str]) -> Result<Self, Error> {
        let mut child = ManagedChild(
            child_command(ffmpeg)
                .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
                .args(input)
                .args(["-ac", "1", "-ar", "8000", "-f", "f32le", "pipe:1"])
                .stdout(Stdio::piped())
                .spawn()?,
        );
        let Some(mut stdout) = child.stdout.take() else {
            stop_child(&mut child, Duration::ZERO)?;
            return Err(Error::Protocol);
        };
        let latest = Arc::new(Mutex::new(None));
        let stopped = Arc::new(AtomicBool::new(false));
        let (output, stop) = (latest.clone(), stopped.clone());
        let worker = std::thread::spawn(move || {
            let mut bytes = [0; PCM_SAMPLES * 4];
            while !stop.load(Ordering::Acquire) && stdout.read_exact(&mut bytes).is_ok() {
                let pcm: Vec<_> = bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                if let Some(frame) = Analysis::from_pcm(&pcm) {
                    *output.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(frame);
                }
            }
        });
        Ok(Self {
            child,
            latest,
            stopped,
            worker: Some(worker),
        })
    }
    pub fn latest(&self) -> Option<Analysis> {
        self.latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }
    pub fn is_running(&mut self) -> Result<bool, Error> {
        Ok(self.child.try_wait()?.is_none())
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.stopped.store(true, Ordering::Release);
        stop_child(&mut self.child, Duration::ZERO)?;
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| Error::Protocol)?;
        }
        Ok(())
    }
}

pub(super) fn validate_monitor(source: &str) -> Result<(), Error> {
    if !cfg!(target_os = "linux") {
        return Err(Error::Unsupported);
    }
    if source.is_empty()
        || source.len() > 256
        || source.starts_with('-')
        || !source
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
