use feishin_spike_ui::SpikeApp;
use std::{
    cell::RefCell,
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
        if let Some(media) = self.media.borrow().as_ref().and_then(Weak::upgrade) {
            media.refresh();
        }
    }
}
type SessionHandlers = Vec<(&'static str, Closure<dyn FnMut()>)>;

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
    fn play(&self) {
        // Both calls happen synchronously in the user gesture before awaiting promises.
        let play = self.audio.play();
        let resume = self.context.resume();
        wasm_bindgen_futures::spawn_local(async move {
            let resumed = match resume {
                Ok(promise) => wasm_bindgen_futures::JsFuture::from(promise).await.is_ok(),
                Err(_) => false,
            };
            let played = match play {
                Ok(promise) => wasm_bindgen_futures::JsFuture::from(promise).await.is_ok(),
                Err(_) => false,
            };
            status(if resumed && played {
                "Playing browser stream; inspect actual audio and media controls."
            } else {
                "Playback rejected: check autoplay permission, CORS, codec and server credentials. Details are not logged."
            });
        });
    }
}
impl Drop for Media {
    fn drop(&mut self) {
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
    }
}

pub async fn start(remote: bool) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
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
            if media.audio.src() != url.href() {
                media.audio.set_src(&url.href());
                media.audio.load();
            }
            media.play();
        })?);
        for event in ["play", "pause", "ended"] {
            let weak = Rc::downgrade(&media);
            events.push(bind(media.audio.as_ref(), event, move |_| {
                if let Some(media) = weak.upgrade() {
                    media.refresh();
                }
            })?);
        }
        events.push(bind(media.audio.as_ref(), "error", |_| status("Media error: stream, credentials, CORS or codec failed. No URL or server error is logged."))?);
        let weak = Rc::downgrade(&media);
        events.push(bind(document.as_ref(), "visibilitychange", move |_| {
            if let Some(media) = weak.upgrade() {
                media.refresh();
            }
        })?);
        for action in ["play", "pause", "stop"] {
            let weak = Rc::downgrade(&media);
            let callback = Closure::wrap(Box::new(move || {
                if let Some(media) = weak.upgrade() {
                    if action == "play" {
                        media.play();
                    } else {
                        let _ = media.audio.pause();
                        if action == "stop" {
                            media.audio.set_current_time(0.0);
                        }
                    }
                }
            }) as Box<dyn FnMut()>);
            if set_media_handler(action, Some(callback.as_ref().unchecked_ref())).is_ok() {
                media.session_handlers.borrow_mut().push((action, callback));
            }
        }
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
    let navigator = web_sys::window().ok_or(JsValue::NULL)?.navigator();
    let session = js_sys::Reflect::get(navigator.as_ref(), &JsValue::from_str("mediaSession"))?;
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
