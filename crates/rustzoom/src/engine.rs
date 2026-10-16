//! The zoom engine: overlay window, Raw Input, capture and rendering on their own thread with a
//! Win32 message loop (ADR 0003). Controlled through [`EngineHandle`], reports back via [`Event`].

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, anyhow};
use rustzoom_core::config::{Config, Hotkey};
use rustzoom_core::geometry::{Rect, scope_layout};
use rustzoom_core::zoom::{ZoomController, ZoomState};
use tracing::{debug, error, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DispatchMessageW, GetMessageW, KillTimer, MSG, PostMessageW, PostQuitMessage,
    SetTimer, TranslateMessage, WM_APP, WM_INPUT, WM_TIMER,
};

use crate::capture::{Capture, WM_APP_FRAME};
use crate::input::{self, InputEvent};
use crate::overlay::{self, Overlay};
use crate::render::Renderer;

/// Posted to the engine window after a [`Command`] was queued.
const WM_APP_COMMAND: u32 = WM_APP + 2;
const PREVIEW_TIMER: usize = 1;
const PREVIEW_MS: u32 = 1500;

/// Which hotkey a binding is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindTarget {
    Zoom,
    Cycle,
}

pub enum Command {
    /// Replace the whole configuration (values are sanitized by the engine).
    ApplyConfig(Config),
    /// Report the next key/mouse press as [`Event::Bound`] instead of zooming. `Esc` cancels.
    StartBinding(BindTarget),
    Quit,
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Bound(BindTarget, Hotkey),
    BindingCancelled,
    /// The engine thread ended (also after a panic).
    Stopped,
}

/// Called on the engine thread for every [`Event`].
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

/// Owner side of the engine thread. Dropping it stops the engine.
pub struct EngineHandle {
    commands: Sender<Command>,
    /// Engine window; raw value because HWND is not Send.
    window: isize,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    /// Starts the engine thread and waits until its window and capture are set up.
    pub fn spawn(config: Config, events: EventSink) -> Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("engine".into())
            .spawn(move || {
                let _stopped = StoppedNotice(events.clone());
                match Engine::start(config, receiver, events) {
                    Ok(window) => {
                        let _ = ready_tx.send(Ok(window));
                        run_message_loop();
                        // Tear down windows and GPU objects while the thread is still fully alive.
                        ENGINE.take();
                        info!("engine stopped");
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                    }
                }
            })
            .context("spawn engine thread")?;
        let window = ready_rx
            .recv()
            .map_err(|_| anyhow!("engine thread died during startup"))??;
        Ok(Self {
            commands,
            window,
            thread: Some(thread),
        })
    }

    pub fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            // SAFETY: posting to the engine's window; if it is already gone the message is
            // simply dropped.
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(self.window as _)),
                    WM_APP_COMMAND,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.send(Command::Quit);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            error!("engine thread panicked");
        }
    }
}

/// Sends [`Event::Stopped`] when the engine thread ends, however it ends.
struct StoppedNotice(EventSink);

impl Drop for StoppedNotice {
    fn drop(&mut self) {
        (self.0)(Event::Stopped);
    }
}

thread_local! {
    /// Engine state for the window procedure. Only exists on the engine thread.
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
}

struct Engine {
    config: Config,
    hmonitor: HMONITOR,
    monitor: Rect,
    zoom: ZoomController,
    overlay: Overlay,
    renderer: Renderer,
    /// Running only while the zoom is enabled.
    capture: Option<Capture>,
    /// Waiting for a key press to bind.
    binding: Option<BindTarget>,
    /// Scope shown briefly after a settings change.
    previewing: bool,
    commands: Receiver<Command>,
    events: EventSink,
}

impl Engine {
    /// Sets up everything on the current (engine) thread and returns the window handle.
    fn start(config: Config, commands: Receiver<Command>, events: EventSink) -> Result<isize> {
        let config = config.sanitized();
        // WinRT (Windows.Graphics.Capture) on this thread.
        // SAFETY: once per thread, before any WinRT call.
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.context("RoInitialize")?;

        let (hmonitor, monitor) = overlay::primary_monitor()?;
        let layout = scope_layout(monitor, config.scope_diameter, config.levels[0]);
        let overlay = Overlay::create(layout.window, Some(wndproc))?;
        let renderer = Renderer::new(overlay.hwnd(), layout.window.width)?;
        input::register(overlay.hwnd())?;

        let window = overlay.hwnd().0 as isize;
        let mut engine = Self {
            zoom: ZoomController::new(config.mode, config.levels.clone()),
            config,
            hmonitor,
            monitor,
            overlay,
            renderer,
            capture: None,
            binding: None,
            previewing: false,
            commands,
            events,
        };
        engine.set_capturing(engine.config.enabled);
        info!(?monitor, ?layout, config = ?engine.config, "engine started");
        ENGINE.set(Some(engine));
        Ok(window)
    }

    fn on_input(&mut self, event: InputEvent) {
        if let Some(target) = self.binding {
            self.on_binding_input(target, event);
            return;
        }
        if !self.config.enabled {
            return;
        }
        let state = match event {
            InputEvent::Pressed(key) if key == self.config.hotkey => self.zoom.on_press(),
            InputEvent::Released(key) if key == self.config.hotkey => self.zoom.on_release(),
            InputEvent::Pressed(key) if Some(key) == self.config.cycle_hotkey => {
                self.zoom.cycle_level()
            }
            _ => return,
        };
        self.update_overlay(state);
    }

    fn on_binding_input(&mut self, target: BindTarget, event: InputEvent) {
        let InputEvent::Pressed(key) = event else {
            return;
        };
        if key == Hotkey::Key(VK_ESCAPE.0) {
            self.binding = None;
            (self.events)(Event::BindingCancelled);
        } else if key.is_bindable() {
            self.binding = None;
            info!(?target, ?key, "hotkey bound");
            (self.events)(Event::Bound(target, key));
        }
        // Left clicks (e.g. in the main window) are ignored and binding continues.
    }

    fn on_frame(&mut self) {
        let Some(capture) = &mut self.capture else {
            return;
        };
        capture.update();
        if self.overlay_visible() {
            self.draw(self.zoom.state());
        }
    }

    fn on_commands(&mut self) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::ApplyConfig(config) => self.reconfigure(config.sanitized()),
                Command::StartBinding(target) => {
                    self.binding = Some(target);
                    // A held zoom would otherwise stay open while binding.
                    self.zoom = ZoomController::new(self.config.mode, self.config.levels.clone());
                    self.update_overlay(self.zoom.state());
                }
                Command::Quit => {
                    // SAFETY: called on the engine thread.
                    unsafe { PostQuitMessage(0) };
                }
            }
        }
    }

    fn reconfigure(&mut self, config: Config) {
        if config == self.config {
            return;
        }
        debug!(?config, "reconfigure");
        let old = std::mem::replace(&mut self.config, config);

        if old.mode != self.config.mode || old.levels != self.config.levels {
            self.zoom = ZoomController::new(self.config.mode, self.config.levels.clone());
        }
        if old.scope_diameter != self.config.scope_diameter {
            let window = scope_layout(self.monitor, self.config.scope_diameter, 1.0).window;
            self.overlay.set_rect(window);
            if let Err(err) = self.renderer.resize(window.width) {
                warn!("resizing scope failed: {err:#}");
            }
        }
        if old.enabled != self.config.enabled {
            self.set_capturing(self.config.enabled);
            self.zoom = ZoomController::new(self.config.mode, self.config.levels.clone());
        }

        let looks_changed = old.scope_diameter != self.config.scope_diameter
            || old.border_width != self.config.border_width
            || old.levels != self.config.levels;
        if looks_changed && self.config.enabled {
            self.start_preview();
        }
        self.update_overlay(self.zoom.state());
    }

    /// Shows the scope for [`PREVIEW_MS`]; every further change restarts the timer.
    fn start_preview(&mut self) {
        self.previewing = true;
        // SAFETY: timer on our own window; re-arming an existing id just resets it.
        unsafe { SetTimer(Some(self.overlay.hwnd()), PREVIEW_TIMER, PREVIEW_MS, None) };
    }

    fn on_preview_timer(&mut self) {
        // SAFETY: timer of our own window.
        let _ = unsafe { KillTimer(Some(self.overlay.hwnd()), PREVIEW_TIMER) };
        self.previewing = false;
        self.update_overlay(self.zoom.state());
    }

    fn set_capturing(&mut self, on: bool) {
        if !on {
            self.capture = None;
            return;
        }
        if self.capture.is_none() {
            match Capture::start(self.renderer.device(), self.hmonitor, self.overlay.hwnd()) {
                Ok(capture) => self.capture = Some(capture),
                Err(err) => error!("starting capture failed: {err:#}"),
            }
        }
    }

    fn overlay_visible(&self) -> bool {
        self.zoom.state().active || self.previewing
    }

    fn update_overlay(&mut self, state: ZoomState) {
        debug!(?state, previewing = self.previewing, "overlay update");
        let visible = self.overlay_visible();
        if visible {
            // Draw before showing, so the window never shows a stale image.
            self.draw(state);
        }
        self.overlay.set_visible(visible);
    }

    fn draw(&self, state: ZoomState) {
        let source = scope_layout(self.monitor, self.config.scope_diameter, state.factor).source;
        let frame = self.capture.as_ref().and_then(Capture::latest_texture);
        if let Err(err) = self
            .renderer
            .draw(frame.as_ref(), source, self.config.border_width)
        {
            warn!("drawing failed: {err:#}");
        }
    }
}

fn run_message_loop() {
    let mut msg = MSG::default();
    // SAFETY: standard message loop on the thread that owns the window. GetMessageW returns -1 on
    // error, so loop only while it is > 0.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        // SAFETY: `msg` was filled by GetMessageW.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Runs `f` on the engine state. Skips silently if the state is not set up yet or already borrowed
/// (re-entrant window messages sent while handling another one).
fn with_engine(f: impl FnOnce(&mut Engine)) {
    ENGINE.with(|cell| {
        if let Ok(mut engine) = cell.try_borrow_mut()
            && let Some(engine) = engine.as_mut()
        {
            f(engine);
        }
    });
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            for event in input::read(lparam) {
                with_engine(|engine| engine.on_input(event));
            }
            // DefWindowProc must still see WM_INPUT to free the raw input buffer.
            // SAFETY: arguments passed through unchanged.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_APP_FRAME => {
            with_engine(Engine::on_frame);
            LRESULT(0)
        }
        WM_APP_COMMAND => {
            with_engine(Engine::on_commands);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == PREVIEW_TIMER => {
            with_engine(Engine::on_preview_timer);
            LRESULT(0)
        }
        // SAFETY: default handling for everything else, arguments passed through unchanged.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
