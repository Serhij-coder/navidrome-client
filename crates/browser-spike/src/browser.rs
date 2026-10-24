use feishin_spike_ui::SpikeApp;
use futures_util::future::{AbortHandle, Abortable, Either, join, select};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{AudioContext, HtmlAudioElement, HtmlCanvasElement, HtmlInputElement};

thread_local! { static ROOT: RefCell<Option<Root>> = const { RefCell::new(None) }; }
struct Root {
    _runner: eframe::WebRunner,
    _media: Option<Rc<Media>>,
    _events: Vec<EventBinding>,
}
struct EventBinding {
    target: web_sys::EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}
impl Drop for EventBinding {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}
fn bind(
    target: &web_sys::EventTarget,
    name: &'static str,
    callback: impl FnMut(web_sys::Event) + 'static,
) -> Result<EventBinding, JsValue> {
    let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(web_sys::Event)>);
    target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
    Ok(EventBinding {
        target: target.clone(),
        name,
        callback,
    })
}
fn status(message: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|doc| doc.get_element_by_id("status"))
    {
        element.set_text_content(Some(message));
    }
}
struct BrowserApp {
    app: Rc<RefCell<SpikeApp>>,
    context: Rc<RefCell<Option<egui_context::Context>>>,
    media: Rc<RefCell<Option<Weak<Media>>>>,
}
// Alias avoids exporting eframe internals to browser callers.
mod egui_context {
    pub type Context = eframe::egui::Context;
}
impl eframe::App for BrowserApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        *self.context.borrow_mut() = Some(ctx.clone());
        self.app.borrow_mut().show(ctx);
        // Canvas widgets have no semantic DOM tree. Mirror the same synthetic
        // table summary for browser assistive technology without exposing inputs.
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("table-status"))
        {
            let summary = self.app.borrow().accessible_table_status();
            if element.text_content().as_deref() != Some(summary.as_str()) {
                element.set_text_content(Some(&summary));
            }
        }
        if let Some(media) = self.media.borrow().as_ref().and_then(Weak::upgrade) {
            media.refresh();
        }
    }
}
type SessionHandlers = Vec<(&'static str, Closure<dyn FnMut(JsValue)>)>;

struct Media {
    audio: HtmlAudioElement,
    context: AudioContext,
    source: web_sys::MediaElementAudioSourceNode,
    analyser: web_sys::AnalyserNode,
    app: Rc<RefCell<SpikeApp>>,
    egui: Rc<RefCell<Option<egui_context::Context>>>,
    interval: RefCell<Option<i32>>,
    tick: RefCell<Option<Closure<dyn FnMut()>>>,
    session_handlers: RefCell<SessionHandlers>,
    generation: Cell<u64>,
    pending_play: RefCell<Option<AbortHandle>>,
    expected_seek: Cell<Option<f64>>,
    seek_generation: Cell<u64>,
    pending_seek: RefCell<Option<AbortHandle>>,
    needs_reload: Cell<bool>,
}
impl Media {
    fn refresh(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let visible = window
            .document()
            .is_some_and(|doc| doc.visibility_state() == web_sys::VisibilityState::Visible);
        let active = visible && !self.audio.paused() && self.app.borrow().wants_analysis();
        if active == self.interval.borrow().is_some() {
            return;
        }
        if let Some(id) = self.interval.borrow_mut().take() {
            window.clear_interval_with_handle(id);
        }
        if active {
            if let Some(tick) = self.tick.borrow().as_ref()
                && let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
                    tick.as_ref().unchecked_ref(),
                    33,
                )
            {
                *self.interval.borrow_mut() = Some(id);
            }
        } else {
            self.app.borrow_mut().analysis = None;
        }
    }
    fn cancel_pending(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(task) = self.pending_play.borrow_mut().take() {
            task.abort();
        }
    }
    fn pause(self: &Rc<Self>, stop: bool) {
        self.cancel_pending();
        let _ = self.audio.pause();
        if stop {
            self.seek(0.0);
        }
        self.refresh();
    }
    fn cancel_seek(&self) {
        self.seek_generation
            .set(self.seek_generation.get().wrapping_add(1));
        self.expected_seek.set(None);
        if let Some(task) = self.pending_seek.borrow_mut().take() {
            task.abort();
        }
    }
    fn fail_seek(&self, message: &str) {
        self.cancel_seek();
        self.cancel_pending();
        self.needs_reload.set(true);
        let _ = self.audio.pause();
        self.refresh();
        self.update_session();
        status(message);
    }
    fn seek(self: &Rc<Self>, seconds: f64) {
        let duration = self.audio.duration();
        if !seconds.is_finite() || !duration.is_finite() || duration <= 0.0 {
            return;
        }
        let target = seconds.clamp(0.0, duration);
        self.cancel_seek();
        if self.audio.error().is_some() {
            self.fail_seek("Seek unavailable after a media error. Reload the stream.");
            return;
        }
        // An unchanged, settled position needs no browser seek event.
        if !self.audio.seeking() && self.audio.current_time() == target {
            self.update_session();
            status("Seek completed in browser stream.");
            return;
        }
        if !self.watch_seek(target) {
            return;
        }
        // Catch browser DOM exceptions without exposing their payload or URL.
        if !matches!(
            js_sys::Reflect::set(self.audio.as_ref(), &"currentTime".into(), &target.into()),
            Ok(true)
        ) {
            self.fail_seek("Seek rejected by the browser. Reload and check stream support.");
            return;
        }
        self.update_session();
    }
    fn observe_element_seek(self: &Rc<Self>) {
        if self.block_failed_resume() {
            return;
        }
        let target = self.audio.current_time();
        let duration = self.audio.duration();
        if !target.is_finite() || !duration.is_finite() || !(0.0..=duration).contains(&target) {
            self.fail_seek("Seek unavailable. Reload and check stream support.");
            return;
        }
        // A callback setter queues this event too. Keep its existing deadline
        // when the observed target agrees; another target replaces the request.
        if self
            .expected_seek
            .get()
            .is_some_and(|expected| (expected - target).abs() < 1e-6)
        {
            return;
        }
        self.watch_seek(target);
    }
    fn watch_seek(self: &Rc<Self>, target: f64) -> bool {
        self.cancel_seek();
        if self.audio.error().is_some() {
            self.fail_seek("Seek unavailable after a media error. Reload the stream.");
            return false;
        }
        let Ok(mut deadline) = BrowserDeadline::new() else {
            self.fail_seek("Seek deadline unavailable. Reload the stream.");
            return false;
        };
        let generation = self.seek_generation.get();
        let weak = Rc::downgrade(self);
        let (abort, registration) = AbortHandle::new_pair();
        *self.pending_seek.borrow_mut() = Some(abort);
        self.expected_seek.set(Some(target));
        status("Seeking browser stream.");
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(
                async move {
                    deadline.wait().await;
                    if let Some(media) = weak.upgrade()
                        && generation == media.seek_generation.get()
                        && media.expected_seek.get().is_some()
                    {
                        media.fail_seek(
                            "Seek timed out. Reload and check stream, codec and range support.",
                        );
                    }
                },
                registration,
            )
            .await;
        });
        true
    }
    fn update_session(&self) {
        let Ok(session) = media_session() else {
            return;
        };
        let _ = js_sys::Reflect::set(
            &session,
            &"playbackState".into(),
            &if self.audio.paused() {
                "paused"
            } else {
                "playing"
            }
            .into(),
        );
        let duration = self.audio.duration();
        let position = self.audio.current_time();
        if duration.is_finite() && duration > 0.0 && position.is_finite() {
            let state = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&state, &"duration".into(), &duration.into());
            let _ = js_sys::Reflect::set(
                &state,
                &"position".into(),
                &position.clamp(0.0, duration).into(),
            );
            let _ = js_sys::Reflect::set(
                &state,
                &"playbackRate".into(),
                &self.audio.playback_rate().into(),
            );
            if let Ok(method) = js_sys::Reflect::get(&session, &"setPositionState".into())
                .and_then(|method| method.dyn_into::<js_sys::Function>())
            {
                let _ = method.call1(&session, &state);
            }
        }
    }
    fn block_failed_resume(&self) -> bool {
        if !self.needs_reload.get() {
            return false;
        }
        self.cancel_pending();
        self.cancel_seek();
        let _ = self.audio.pause();
        self.refresh();
        self.update_session();
        status("Playback requires a visible reload after a failed operation.");
        true
    }
    fn play(self: &Rc<Self>) {
        if self.block_failed_resume() {
            return;
        }
        self.cancel_pending();
        let generation = self.generation.get();
        status("Starting browser stream.");
        // Both calls stay within the original user gesture.
        let play = self.audio.play();
        let resume = self.context.resume();
        let weak = Rc::downgrade(self);
        let (abort, registration) = AbortHandle::new_pair();
        *self.pending_play.borrow_mut() = Some(abort);
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(async move {
                let result = async move {
                    let (resumed, played) = join(promise_succeeded(resume), promise_succeeded(play)).await;
                    resumed && played
                };
                let success = if let Ok(mut deadline) = BrowserDeadline::new() {
                    matches!(select(Box::pin(result), Box::pin(deadline.wait())).await, Either::Left((true, _)))
                } else { false };
                if let Some(media) = weak.upgrade()
                    && generation == media.generation.get()
                {
                    media.pending_play.borrow_mut().take();
                    if !success { media.needs_reload.set(true); let _ = media.audio.pause(); }
                    status(if success && !media.audio.paused() {
                        "Playing browser stream; inspect actual audio and media controls."
                    } else {
                        "Playback rejected or timed out: check autoplay permission, CORS, codec and server credentials. Details are not logged."
                    });
                    media.refresh();
                    media.update_session();
                }
            }, registration).await;
        });
    }
}
impl Drop for Media {
    fn drop(&mut self) {
        self.cancel_pending();
        self.cancel_seek();
        if let Some(window) = web_sys::window() {
            if let Some(id) = self.interval.borrow_mut().take() {
                window.clear_interval_with_handle(id);
            }
            for (action, _) in self.session_handlers.borrow_mut().drain(..) {
                let _ = set_media_handler(action, None);
            }
        }
        let _ = self.audio.pause();
        self.audio.set_src("");
        self.audio.load();
        let _ = self.source.disconnect();
        let _ = self.analyser.disconnect();
        let _ = self.context.close();
        if let Ok(session) = media_session() {
            let _ = js_sys::Reflect::set(&session, &"metadata".into(), &JsValue::NULL);
            let _ = js_sys::Reflect::set(&session, &"playbackState".into(), &"none".into());
        }
    }
}

pub async fn start(remote: bool) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    if document.get_element_by_id("table-status").is_none() {
        let summary = document.create_element("p")?;
        summary.set_id("table-status");
        summary.set_attribute("role", "status")?;
        summary.set_attribute("aria-live", "polite")?;
        summary.set_attribute("aria-atomic", "true")?;
        document
            .query_selector("main")?
            .ok_or_else(|| JsValue::from_str("main unavailable"))?
            .append_child(&summary)?;
    }
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or_else(|| JsValue::from_str("canvas unavailable"))?
        .dyn_into::<HtmlCanvasElement>()?;
    let app = Rc::new(RefCell::new(SpikeApp::new(
        &eframe::egui::Context::default(),
        remote,
    )));
    let egui = Rc::new(RefCell::new(None));
    let media_link = Rc::new(RefCell::new(None));
    let link = media_link.clone();
    let runner = eframe::WebRunner::new();
    let (ui, context) = (app.clone(), egui.clone());
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |_| {
                Ok(Box::new(BrowserApp {
                    app: ui,
                    context,
                    media: link,
                }))
            }),
        )
        .await?;
    let mut events = Vec::new();
    let media = if remote {
        if let Some(controls) = document.get_element_by_id("media-controls") {
            controls.set_attribute("hidden", "")?;
        }
        status(
            "Remote presentation experiment only. Desktop connection and protocol are P11; this root owns no audio or queue.",
        );
        None
    } else {
        let audio = document
            .get_element_by_id("audio")
            .ok_or_else(|| JsValue::from_str("audio unavailable"))?
            .dyn_into::<HtmlAudioElement>()?;
        audio.set_cross_origin(Some("anonymous"));
        let context = AudioContext::new()?;
        let source = context.create_media_element_source(&audio)?;
        let analyser = context.create_analyser()?;
        analyser.set_fft_size(256);
        source.connect_with_audio_node(&context.destination())?;
        source.connect_with_audio_node(&analyser)?;
        let media = Rc::new(Media {
            audio,
            context,
            source,
            analyser,
            app,
            egui,
            interval: RefCell::new(None),
            tick: RefCell::new(None),
            session_handlers: RefCell::new(Vec::new()),
            generation: Cell::new(0),
            pending_play: RefCell::new(None),
            expected_seek: Cell::new(None),
            seek_generation: Cell::new(0),
            pending_seek: RefCell::new(None),
            needs_reload: Cell::new(false),
        });
        let weak: Weak<Media> = Rc::downgrade(&media);
        *media.tick.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            if let Some(media) = weak.upgrade() {
                let mut pcm = [0.0; 256];
                media.analyser.get_float_time_domain_data(&mut pcm);
                media.app.borrow_mut().analysis = feishin_spike_ui::analysis_from_pcm(&pcm);
                if let Some(ctx) = media.egui.borrow().as_ref() {
                    ctx.request_repaint();
                }
            }
        }) as Box<dyn FnMut()>));
        let input = document
            .get_element_by_id("stream")
            .ok_or_else(|| JsValue::from_str("stream input unavailable"))?
            .dyn_into::<HtmlInputElement>()?;
        let play = document
            .get_element_by_id("play")
            .ok_or_else(|| JsValue::from_str("play unavailable"))?;
        let weak = Rc::downgrade(&media);
        events.push(bind(play.as_ref(), "click", move |_| {
            let Some(media) = weak.upgrade() else {
                return;
            };
            let value = input.value();
            let Ok(url) = web_sys::Url::new(&value) else {
                status("Enter an absolute HTTP(S) stream URL.");
                return;
            };
            if !["http:", "https:"].contains(&url.protocol().as_str())
                || !url.username().is_empty()
                || !url.password().is_empty()
            {
                status("Only HTTP(S) URLs without embedded username/password are accepted.");
                return;
            }
            let reload = media.needs_reload.replace(false);
            if media.audio.src() != url.href() || reload || media.audio.error().is_some() {
                media.cancel_pending();
                media.cancel_seek();
                media.audio.set_src(&url.href());
                media.audio.load();
            }
            media.play();
        })?);
        for event in [
            "play",
            "playing",
            "pause",
            "ended",
            "loadedmetadata",
            "timeupdate",
            "ratechange",
            "seeking",
            "seeked",
        ] {
            let weak = Rc::downgrade(&media);
            events.push(bind(media.audio.as_ref(), event, move |_| {
                if let Some(media) = weak.upgrade() {
                    if matches!(event, "play" | "playing") && media.block_failed_resume() {
                        return;
                    }
                    if event == "pause" && media.audio.paused() {
                        media.cancel_pending();
                    }
                    if event == "seeking" {
                        media.observe_element_seek();
                    }
                    if event == "seeked" && !media.audio.seeking()
                        && let Some(expected) = media.expected_seek.take()
                    {
                        let position = media.audio.current_time();
                        if !position.is_finite() || (position - expected).abs() > 0.25 {
                            media.fail_seek("Seek did not reach the requested position. Reload and check stream, codec and range support.");
                        } else {
                            media.cancel_seek();
                            status("Seek completed in browser stream.");
                        }
                    }
                    media.refresh();
                    media.update_session();
                }
            })?);
        }
        let weak = Rc::downgrade(&media);
        events.push(bind(media.audio.as_ref(), "error", move |_| {
            if let Some(media) = weak.upgrade() && media.audio.error().is_some() {
                media.cancel_pending();
                media.cancel_seek();
                media.needs_reload.set(true);
                media.refresh();
                status("Media error: stream, credentials, CORS or codec failed. No URL or server error is logged.");
            }
        })?);
        let weak = Rc::downgrade(&media);
        events.push(bind(document.as_ref(), "visibilitychange", move |_| {
            if let Some(media) = weak.upgrade() {
                media.refresh();
            }
        })?);
        // No next/previous registration: this shell has no playback queue.
        for action in [
            "play",
            "pause",
            "stop",
            "seekto",
            "seekbackward",
            "seekforward",
        ] {
            let weak = Rc::downgrade(&media);
            let callback = Closure::wrap(Box::new(move |details: JsValue| {
                if let Some(media) = weak.upgrade() {
                    match action {
                        "play" => media.play(),
                        "pause" => media.pause(false),
                        "stop" => media.pause(true),
                        "seekto" => {
                            if let Some(seconds) = detail_number(&details, "seekTime") {
                                media.seek(seconds);
                            }
                        }
                        "seekbackward" | "seekforward" => {
                            let offset = detail_number(&details, "seekOffset").unwrap_or(5.0);
                            if offset.is_finite() && offset >= 0.0 {
                                media.seek(
                                    media.audio.current_time()
                                        + if action == "seekbackward" {
                                            -offset
                                        } else {
                                            offset
                                        },
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }) as Box<dyn FnMut(JsValue)>);
            if set_media_handler(action, Some(callback.as_ref().unchecked_ref())).is_ok() {
                media.session_handlers.borrow_mut().push((action, callback));
            }
        }
        set_fixture_metadata();
        status(
            "Paste a disposable authenticated HTTP(S) stream URL, then Play. URL stays only in this tab. Cookies use anonymous CORS; token query URLs require server CORS permission.",
        );
        *media_link.borrow_mut() = Some(Rc::downgrade(&media));
        Some(media)
    };
    ROOT.with(|root| {
        *root.borrow_mut() = Some(Root {
            _runner: runner,
            _media: media,
            _events: events,
        });
    });
    Ok(())
}

// Catch optional-browser API failures instead of allowing an imported JS exception
// to abort the WASM shell. Keep this compatibility binding in the browser crate.
fn set_media_handler(action: &str, callback: Option<&js_sys::Function>) -> Result<(), JsValue> {
    let session = media_session()?;
    let method = js_sys::Reflect::get(&session, &JsValue::from_str("setActionHandler"))?
        .dyn_into::<js_sys::Function>()?;
    method.call2(
        &session,
        &JsValue::from_str(action),
        callback.map_or(&JsValue::NULL, |function| function.as_ref()),
    )?;
    Ok(())
}

#[wasm_bindgen]
pub fn set_analysis_visible(visible: bool) {
    ROOT.with(|root| {
        if let Some(root) = root.borrow().as_ref()
            && let Some(media) = root._media.as_ref()
        {
            media.app.borrow_mut().set_analysis_visible(visible);
            media.refresh();
            if let Some(ctx) = media.egui.borrow().as_ref() {
                ctx.request_repaint();
            }
        }
    });
}
#[wasm_bindgen]
pub fn analysis_peak() -> f32 {
    ROOT.with(|root| {
        root.borrow()
            .as_ref()
            .and_then(|root| root._media.as_ref())
            .and_then(|media| {
                media
                    .app
                    .borrow()
                    .analysis
                    .as_ref()
                    .map(|frame| frame.bands.iter().copied().fold(0.0, f32::max))
            })
            .unwrap_or(0.0)
    })
}

fn media_session() -> Result<JsValue, JsValue> {
    let navigator = web_sys::window().ok_or(JsValue::NULL)?.navigator();
    js_sys::Reflect::get(navigator.as_ref(), &"mediaSession".into())
}
fn detail_number(details: &JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(details, &key.into()).ok()?.as_f64()
}
fn set_fixture_metadata() {
    let _ = (|| -> Result<(), JsValue> {
        let window = web_sys::window().ok_or(JsValue::NULL)?;
        let constructor = js_sys::Reflect::get(window.as_ref(), &"MediaMetadata".into())?
            .dyn_into::<js_sys::Function>()?;
        let data = js_sys::Object::new();
        js_sys::Reflect::set(&data, &"title".into(), &"P01 disposable stream".into())?;
        let args = js_sys::Array::new();
        args.push(&data);
        let metadata = js_sys::Reflect::construct(&constructor, &args)?;
        js_sys::Reflect::set(&media_session()?, &"metadata".into(), &metadata)?;
        Ok(())
    })();
}
async fn promise_succeeded(promise: Result<js_sys::Promise, JsValue>) -> bool {
    match promise {
        Ok(promise) => wasm_bindgen_futures::JsFuture::from(promise).await.is_ok(),
        Err(_) => false,
    }
}

// The timeout is owned by the current, abortable media operation; cancellation clears it.
struct BrowserDeadline {
    handle: i32,
    _callback: Closure<dyn FnMut()>,
    receiver: futures_channel::oneshot::Receiver<()>,
}
impl BrowserDeadline {
    fn new() -> Result<Self, JsValue> {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let mut sender = Some(sender);
        let callback = Closure::wrap(Box::new(move || {
            if let Some(sender) = sender.take() {
                let _ = sender.send(());
            }
        }) as Box<dyn FnMut()>);
        let handle = web_sys::window()
            .ok_or(JsValue::NULL)?
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                8_000,
            )?;
        Ok(Self {
            handle,
            _callback: callback,
            receiver,
        })
    }
    async fn wait(&mut self) {
        let _ = (&mut self.receiver).await;
    }
}
impl Drop for BrowserDeadline {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            window.clear_timeout_with_handle(self.handle);
        }
    }
}

#[wasm_bindgen]
pub fn analysis_running() -> bool {
    ROOT.with(|root| {
        root.borrow()
            .as_ref()
            .and_then(|root| root._media.as_ref())
            .is_some_and(|media| media.interval.borrow().is_some())
    })
}

#[wasm_bindgen]
pub fn shutdown() {
    ROOT.with(|root| {
        if let Some(root) = root.borrow_mut().take() {
            root._runner.destroy();
        }
    });
}
