mod preferences;
use feishin_native_spike::{
    AnalysisSession, AnalysisSource, AnalysisStatus, LogEvent, log_event, write_tone,
};
use feishin_spike_ui::SpikeApp;
use std::{
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "arguments must be valid Unicode")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if args.first().is_some_and(|arg| arg == "--tone") && args.len() == 2 {
        write_tone(Path::new(&args[1]), 5, 750)?;
        return Ok(());
    }
    if args
        .first()
        .is_some_and(|arg| ["--ffplay-probe", "--ffplay-probe-null"].contains(&arg.as_str()))
        && args.len() == 3
    {
        let mut probe = feishin_native_spike::FfplayProbe::start_local(
            Path::new(&args[1]),
            Path::new(&args[2]),
            0.0,
            1,
            if args[0] == "--ffplay-probe-null" {
                feishin_native_spike::FfplayOutput::Dummy
            } else {
                feishin_native_spike::FfplayOutput::System
            },
        )?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(success) = probe.poll()? {
                if !success {
                    return Err("alternative decoder failed".into());
                }
                break;
            }
            if Instant::now() >= deadline {
                probe.shutdown()?;
                return Err("alternative decoder probe deadline".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        println!("{}", log_event(LogEvent::Stopped, 1));
        return Ok(());
    }
    if args
        .first()
        .is_some_and(|arg| ["--mpv-probe", "--mpv-probe-null"].contains(&arg.as_str()))
        && args.len() == 3
    {
        #[cfg(any(unix, windows))]
        {
            let mut mpv = feishin_native_spike::Mpv::start(
                Path::new(&args[1]),
                1,
                if args[0] == "--mpv-probe-null" {
                    feishin_native_spike::AudioOutput::Null
                } else {
                    feishin_native_spike::AudioOutput::System
                },
            )?;
            mpv.load_local(Path::new(&args[2]), false)?;
            std::thread::sleep(Duration::from_secs(2));
            mpv.pause(true)?;
            mpv.seek(1.0)?;
            mpv.pause(false)?;
            std::thread::sleep(Duration::from_secs(2));
            mpv.shutdown()?;
            println!("{}", log_event(LogEvent::Stopped, 1));
            return Ok(());
        }
        #[cfg(not(any(unix, windows)))]
        return Err("mpv IPC probe not implemented on this platform".into());
    }
    let mut profile_root: Option<PathBuf> = None;
    let mut capture = None;
    let mut smoke = false;
    let mut hidden_smoke = false;
    let mut renderer_wgpu = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--profile-root" if index + 1 < args.len() && profile_root.is_none() => {
                let path = PathBuf::from(&args[index + 1]);
                if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) { return Err("invalid profile root".into()); }
                profile_root = Some(path); index += 2;
            }
            "--capture-monitor" if index + 1 < args.len() => {
                capture = Some(AnalysisSource::Monitor(args[index + 1].clone())); index += 2;
            }
            "--analysis-test-tone" => { capture = Some(AnalysisSource::TestTone); index += 1; }
            "--smoke" => { smoke = true; index += 1; }
            "--hidden-smoke" => { smoke = true; hidden_smoke = true; index += 1; }
            "--wgpu" => { renderer_wgpu = true; index += 1; }
            _ => return Err("usage: feishin-desktop [--profile-root ABS_PATH] [--smoke|--hidden-smoke] [--wgpu] [--analysis-test-tone|--capture-monitor SOURCE] | --tone NEW.wav | --mpv-probe[-null] MPV FILE.wav | --ffplay-probe[-null] FFPLAY FILE.wav".into()),
        }
    }
    let renderer = if renderer_wgpu {
        #[cfg(feature = "wgpu")]
        {
            eframe::Renderer::Wgpu
        }
        #[cfg(not(feature = "wgpu"))]
        {
            return Err("build with --no-default-features --features wgpu".into());
        }
    } else {
        #[cfg(feature = "glow")]
        {
            eframe::Renderer::Glow
        }
        #[cfg(not(feature = "glow"))]
        {
            return Err("wgpu-only builds require --wgpu".into());
        }
    };
    let options = eframe::NativeOptions {
        renderer,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_visible(!hidden_smoke),
        ..Default::default()
    };
    println!("{}", log_event(LogEvent::Started, 0));
    eframe::run_native(
        "Feishin Rust P01",
        options,
        Box::new(move |cc| {
            let mut ui = SpikeApp::new(&cc.egui_ctx, false);
            ui.show_theme_controls = profile_root.is_none();
            let preferences = profile_root.map(preferences::Controller::new).transpose()?;
            if capture.is_some() {
                ui.enable_analysis();
            }
            let capture = capture
                .map(|source| {
                    let ctx = cc.egui_ctx.clone();
                    AnalysisSession::new("ffmpeg".into(), source, move || ctx.request_repaint())
                })
                .transpose()?;
            Ok(Box::new(DesktopProbe {
                ui,
                preferences,
                capture,
                started: Instant::now(),
                smoke,
                hidden_smoke,
                frames: 0,
                first_paint_ms: None,
            }))
        }),
    )?;
    println!("{}", log_event(LogEvent::Stopped, 0));
    Ok(())
}

struct DesktopProbe {
    ui: SpikeApp,
    preferences: Option<preferences::Controller>,
    capture: Option<AnalysisSession>,
    started: Instant,
    smoke: bool,
    hidden_smoke: bool,
    frames: u64,
    first_paint_ms: Option<u128>,
}
impl eframe::App for DesktopProbe {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.frames += 1;
        self.first_paint_ms
            .get_or_insert(self.started.elapsed().as_millis());
        let hidden =
            self.hidden_smoke || ctx.input(|input| input.viewport().minimized.unwrap_or(false));
        if let Some(capture) = &self.capture {
            capture.set_active(!hidden && self.ui.wants_analysis());
            if let Some(frame) = capture.latest() {
                self.ui.analysis = Some(frame);
            }
            if capture.status() == AnalysisStatus::Failed {
                self.ui.notice = Some(
                    "Analysis source failed. Check permission/source, then toggle Analysis preset off/on to retry.",
                );
            } else {
                self.ui.notice = None;
            }
        }
        if let Some(preferences) = &mut self.preferences {
            if !preferences.stopped() && ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
                preferences.close();
            }
            preferences.tick(ctx);
            preferences.show(ctx);
            if preferences.stopped() {
                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            }
        }
        self.ui.show(ctx);
        if let Some(capture) = &self.capture {
            capture.set_active(!hidden && self.ui.wants_analysis());
        }
        if hidden || !self.ui.wants_analysis() {
            self.ui.analysis = None;
        }
        if self.smoke {
            if self.started.elapsed() >= Duration::from_millis(500) {
                println!(
                    "{{\"probe\":\"shell-smoke\",\"update_calls\":{},\"first_update_ms\":{},\"visible_rows\":{},\"hidden\":{}}}",
                    self.frames,
                    self.first_paint_ms.unwrap_or_default(),
                    self.ui.rows_drawn,
                    hidden
                );
                self.smoke = false;
                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }
}
