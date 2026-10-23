use feishin_spike_model::FadeCurve;

#[test]
fn gain_coefficients_match_the_original_active_handler_replay() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/p01/crossfade/upstream-gains.json"
    ))
    .unwrap();
    let samples = fixture["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 198);
    for sample in samples {
        let curve = match sample["style"].as_str().unwrap() {
            "linear" => FadeCurve::Linear,
            "sCurve" => FadeCurve::SCurve,
            "exponential" => FadeCurve::UpstreamExponential,
            // Active source maps all legacy/unknown styles to equal power.
            _ => FadeCurve::EqualPower,
        };
        let (outgoing, incoming) = curve
            .coefficients(sample["progress"].as_f64().unwrap())
            .unwrap();
        assert!((outgoing - sample["outgoing"].as_f64().unwrap()).abs() < 1e-12);
        assert!((incoming - sample["incoming"].as_f64().unwrap()).abs() < 1e-12);
        let (out_gain, in_gain) = curve
            .web_gain_scalars(
                sample["progress"].as_f64().unwrap(),
                sample["master_fraction"].as_f64().unwrap(),
            )
            .unwrap();
        assert!((out_gain - sample["web_outgoing_gain"].as_f64().unwrap()).abs() < 1e-12);
        assert!((in_gain - sample["web_incoming_gain"].as_f64().unwrap()).abs() < 1e-12);
    }
}

#[test]
fn envelopes_have_expected_bounds_and_reject_nonfinite_progress() {
    for curve in [
        FadeCurve::Linear,
        FadeCurve::EqualPower,
        FadeCurve::SCurve,
        FadeCurve::UpstreamExponential,
    ] {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(curve.coefficients(invalid), None);
            assert_eq!(curve.web_gain_scalars(0.5, invalid), None);
        }
        for index in 0..=1000 {
            let (outgoing, incoming) = curve.coefficients(f64::from(index) / 1000.0).unwrap();
            assert!((0.0..=1.0).contains(&outgoing) && (0.0..=1.0).contains(&incoming));
            match curve {
                FadeCurve::EqualPower => {
                    assert!((outgoing * outgoing + incoming * incoming - 1.0).abs() < 1e-12)
                }
                FadeCurve::Linear | FadeCurve::SCurve => {
                    assert!((outgoing + incoming - 1.0).abs() < 1e-12)
                }
                FadeCurve::UpstreamExponential => {}
            }
        }
    }
    assert_eq!(
        FadeCurve::UpstreamExponential.coefficients(0.0),
        Some((0.0, 0.0))
    );
    let (outgoing, incoming) = FadeCurve::UpstreamExponential.coefficients(1.0).unwrap();
    assert_eq!(outgoing, 1.0);
    assert!(incoming > 0.99 && incoming < 1.0);
}

#[test]
fn assigned_equal_power_gains_include_the_source_squared_slider_mapping() {
    let (outgoing, incoming) = FadeCurve::EqualPower.web_gain_scalars(0.5, 1.0).unwrap();
    assert!((outgoing - 0.5).abs() < 1e-12 && (incoming - 0.5).abs() < 1e-12);
    for invalid in [-0.1, 1.1] {
        assert_eq!(FadeCurve::Linear.web_gain_scalars(0.5, invalid), None);
    }
}
