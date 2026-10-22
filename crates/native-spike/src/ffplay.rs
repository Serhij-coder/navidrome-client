use super::*;

#[derive(Clone, Copy)]
pub enum FfplayOutput {
    System,
    /// SDL's dummy audio driver validates decode/lifecycle, not a sound device.
    Dummy,
}

/// Explicit alternative-engine evaluation, not the production fallback decision.
/// ffplay has no control IPC: dynamic seek/pause/device control remain unsupported.
pub struct FfplayProbe {
    child: ManagedChild,
    generation: u64,
    active: bool,
}

impl FfplayProbe {
    pub fn start_local(
        binary: &Path,
        file: &Path,
        start_seconds: f64,
        generation: u64,
        output: FfplayOutput,
    ) -> Result<Self, Error> {
        if !start_seconds.is_finite() || !(0.0..=86_400.0).contains(&start_seconds) {
            return Err(Error::InvalidInput);
        }
        let path = file.canonicalize()?;
        if !path.is_file() {
            return Err(Error::InvalidInput);
        }
        let mut command = child_command(binary);
        command
            .args([
                "-nostats",
                "-hide_banner",
                "-loglevel",
                "error",
                "-nodisp",
                "-autoexit",
                "-ss",
            ])
            .arg(start_seconds.to_string())
            .arg("-i")
            .arg(path);
        if matches!(output, FfplayOutput::Dummy) {
            command.env("SDL_AUDIODRIVER", "dummy");
        }
        Ok(Self {
            child: ManagedChild(command.spawn()?),
            generation,
            active: true,
        })
    }

    pub fn accepts_generation(&self, generation: u64) -> bool {
        self.active && self.generation == generation
    }

    /// None means still decoding; Some means process completion, not device verification.
    pub fn poll(&mut self) -> Result<Option<bool>, Error> {
        match self.child.try_wait()? {
            Some(status) => {
                self.active = false;
                Ok(Some(status.success()))
            }
            None => Ok(None),
        }
    }

    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.active = false;
        stop_child(&mut self.child, Duration::ZERO)
    }
}

impl Drop for FfplayProbe {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
