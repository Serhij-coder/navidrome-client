//! P01 gain-envelope feasibility values from the pinned active Feishin player.
//! This is not an application playback/queue reducer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadeCurve {
    Linear,
    EqualPower,
    SCurve,
    /// Preserves the observed inverted outgoing curve, not its source comment.
    /// Both gains rise. This compatibility probe is not a production choice.
    UpstreamExponential,
}

impl FadeCurve {
    /// Normalized outgoing/incoming slider coefficients before the engine volume mapping. Reject non-finite
    /// input at the boundary; finite progress is clamped like the source easing.
    pub fn coefficients(self, progress: f64) -> Option<(f64, f64)> {
        if !progress.is_finite() {
            return None;
        }
        let t = progress.clamp(0.0, 1.0);
        Some(match self {
            Self::Linear => (1.0 - t, t),
            Self::EqualPower => {
                let angle = std::f64::consts::FRAC_PI_2 * t;
                (angle.cos(), angle.sin())
            }
            Self::SCurve => {
                let eased = t * t * (3.0 - 2.0 * t);
                (1.0 - eased, eased)
            }
            Self::UpstreamExponential => {
                let end = (-5.0_f64).exp();
                let current = (-5.0 * t).exp();
                (1.0 - (current - end) / (1.0 - end), 1.0 - current)
            }
        })
    }
}

impl FadeCurve {
    /// Source-faithful scalars assigned to the browser media element after the
    /// active engine squares its normalized slider volume. This is not a claim
    /// about browser/device acoustics or mpv's separate volume-percentage law.
    pub fn web_gain_scalars(self, progress: f64, master_fraction: f64) -> Option<(f64, f64)> {
        if !master_fraction.is_finite() || !(0.0..=1.0).contains(&master_fraction) {
            return None;
        }
        let (outgoing, incoming) = self.coefficients(progress)?;
        Some((
            (outgoing * master_fraction).powi(2),
            (incoming * master_fraction).powi(2),
        ))
    }
}
