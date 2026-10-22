use feishin_native_spike::{Capture, LogEvent, log_event, write_tone};
use feishin_spike_ui::SpikeApp;
use std::{
    path::Path,
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
        .is_some_and(|arg| ["--mpv-probe", "--mpv-probe-null"].contains(&arg.as_str()))
        && args.len() == 3
    {
        #[cfg(unix)]
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
        #[cfg(not(unix))]
        return Err("mpv IPC probe not implemented on this platform".into());
    }
    let mut capture = None;
    let mut smoke = false;
    let mut hidden_smoke = false;
    let mut renderer_wgpu = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--capture-monitor" if index + 1 < args.len() => {
                capture = Some(Capture::start_monitor(Path::new("ffmpeg"), &args[index + 1])?); index += 2;
            }
            "--analysis-test-tone" => { capture = Some(Capture::start_test_tone(Path::new("ffmpeg"))?); index += 1; }
            "--smoke" => { smoke = true; index += 1; }
            "--hidden-smoke" => { smoke = true; hidden_smoke = true; index += 1; }
            "--wgpu" => { renderer_wgpu = true; index += 1; }
            _ => return Err("usage: feishin-desktop [--smoke|--hidden-smoke] [--wgpu] [--analysis-test-tone|--capture-monitor SOURCE] | --tone NEW.wav | --mpv-probe[-null] MPV FILE.wav".into()),
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
            if capture.is_some() {
                ui.enable_analysis();
            }
            Ok(Box::new(DesktopProbe {
                ui,
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
    capture: Option<Capture>,
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
        if hidden {
            self.capture = None;
            self.ui.analysis = None;
        }
        if let Some(capture) = &mut self.capture {
            if let Some(frame) = capture.latest() {
                self.ui.analysis = Some(frame);
            }
            if !matches!(capture.is_running(), Ok(true)) {
                self.capture = None;
                self.ui.notice = Some(
                    "Analysis source exited or access was denied. Check the selected source and permission; restart explicitly to retry.",
                );
            }
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        self.ui.show(ctx);
        if !self.ui.wants_analysis() {
            self.capture = None;
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
