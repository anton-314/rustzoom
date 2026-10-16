//! The main window (ADR 0003): edits the config, hands every change to the engine and saves it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui;
use rustzoom_core::config::{
    self, Config, Language, MAX_BORDER, MAX_DIAMETER, MAX_FACTOR, MAX_LEVELS, MIN_DIAMETER,
    MIN_FACTOR,
};
use rustzoom_core::i18n::{Text, language_name, text};
use rustzoom_core::names::hotkey_name;
use rustzoom_core::zoom::ZoomMode;
use tracing::{info, warn};
use windows::Win32::Globalization::GetUserDefaultUILanguage;

use crate::engine::{BindTarget, Command, EngineHandle, Event};

/// Changes are written at most this often while the user drags a slider.
const SAVE_DELAY: Duration = Duration::from_millis(500);
const LANG_GERMAN: u16 = 0x07;

/// Opens the main window and blocks until it is closed.
pub fn run(config: Config, path: PathBuf) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rustzoom")
            .with_inner_size([440.0, 520.0])
            .with_min_inner_size([380.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rustzoom",
        options,
        Box::new(move |cc| {
            let (events_tx, events) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            let sink = Arc::new(move |event| {
                let _ = events_tx.send(event);
                ctx.request_repaint();
            });
            // Started only now, after winit registered its Raw Input, so the engine's
            // registration wins (ADR 0003).
            let engine = EngineHandle::spawn(config.clone(), sink)?;
            Ok(Box::new(MainWindow::new(config, path, engine, events)))
        }),
    )
    .map_err(|err| anyhow::anyhow!("main window: {err}"))
}

struct MainWindow {
    config: Config,
    /// What the engine currently has.
    sent: Config,
    /// What is on disk.
    saved: Config,
    path: PathBuf,
    /// First unsaved change, if any.
    dirty_since: Option<Instant>,
    /// Hotkey currently waiting for a key press.
    binding: Option<BindTarget>,
    engine: EngineHandle,
    events: Receiver<Event>,
}

impl MainWindow {
    fn new(config: Config, path: PathBuf, engine: EngineHandle, events: Receiver<Event>) -> Self {
        Self {
            sent: config.clone(),
            saved: config.clone(),
            config,
            path,
            dirty_since: None,
            binding: None,
            engine,
            events,
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Bound(BindTarget::Zoom, key) => self.config.hotkey = key,
                Event::Bound(BindTarget::Cycle, key) => self.config.cycle_hotkey = Some(key),
                Event::BindingCancelled => {}
                Event::Stopped => {
                    warn!("engine stopped, closing main window");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if matches!(event, Event::Bound(..) | Event::BindingCancelled) {
                self.binding = None;
            }
        }
    }

    /// Current key name plus "Change…", or the "press a key" prompt while binding.
    fn key_row(&mut self, ui: &mut egui::Ui, lang: Language, target: BindTarget) {
        let current = match target {
            BindTarget::Zoom => Some(self.config.hotkey),
            BindTarget::Cycle => self.config.cycle_hotkey,
        };
        ui.horizontal(|ui| {
            if self.binding == Some(target) {
                ui.label(egui::RichText::new(text(lang, Text::PressKey)).italics());
                return;
            }
            let name = current.map_or_else(|| text(lang, Text::NoKey).to_owned(), hotkey_name);
            ui.label(egui::RichText::new(name).monospace());
            let change = ui.add_enabled(
                self.binding.is_none(),
                egui::Button::new(text(lang, Text::Change)),
            );
            if change.clicked() {
                self.binding = Some(target);
                self.engine.send(Command::StartBinding(target));
            }
            if target == BindTarget::Cycle
                && current.is_some()
                && ui.small_button(text(lang, Text::Clear)).clicked()
            {
                self.config.cycle_hotkey = None;
            }
        });
    }

    fn language(&self) -> Language {
        self.config.language.unwrap_or_else(system_language)
    }

    /// Sends changes to the engine right away and saves them a little later.
    fn sync(&mut self, ctx: &egui::Context) {
        self.config = self.config.clone().sanitized();
        if self.config != self.sent {
            self.sent = self.config.clone();
            self.engine.send(Command::ApplyConfig(self.config.clone()));
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        if let Some(since) = self.dirty_since {
            let elapsed = since.elapsed();
            if elapsed >= SAVE_DELAY {
                self.save();
            } else {
                ctx.request_repaint_after(SAVE_DELAY - elapsed);
            }
        }
    }

    fn save(&mut self) {
        self.dirty_since = None;
        if self.config == self.saved {
            return;
        }
        match config::save(&self.path, &self.config) {
            Ok(()) => {
                self.saved = self.config.clone();
                info!("config saved");
            }
            Err(err) => warn!("saving {} failed: {err}", self.path.display()),
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let lang = self.language();
        let tr = |t| text(lang, t);

        ui.heading("rustzoom");
        ui.add_space(4.0);
        ui.checkbox(
            &mut self.config.enabled,
            egui::RichText::new(tr(Text::ZoomEnabled)).strong(),
        );
        ui.separator();

        egui::Grid::new("settings")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label(tr(Text::ZoomKey));
                self.key_row(ui, lang, BindTarget::Zoom);
                ui.end_row();

                ui.label(tr(Text::Mode));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.config.mode, ZoomMode::Hold, tr(Text::ModeHold));
                    ui.radio_value(
                        &mut self.config.mode,
                        ZoomMode::Toggle,
                        tr(Text::ModeToggle),
                    );
                });
                ui.end_row();

                ui.label(tr(Text::Levels));
                ui.vertical(|ui| self.levels(ui, lang));
                ui.end_row();

                ui.label(tr(Text::CycleKey));
                self.key_row(ui, lang, BindTarget::Cycle);
                ui.end_row();

                ui.label(tr(Text::ScopeSize));
                ui.add(
                    egui::Slider::new(&mut self.config.scope_diameter, MIN_DIAMETER..=MAX_DIAMETER)
                        .step_by(0.01)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
                ui.end_row();

                ui.label(tr(Text::BorderWidth));
                ui.add(
                    egui::Slider::new(&mut self.config.border_width, 0.0..=MAX_BORDER)
                        .step_by(1.0)
                        .suffix(" px"),
                );
                ui.end_row();

                ui.label(tr(Text::Language));
                let selected = self
                    .config
                    .language
                    .map_or_else(|| tr(Text::LanguageSystem), language_name);
                egui::ComboBox::from_id_salt("language")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.config.language,
                            None,
                            tr(Text::LanguageSystem),
                        );
                        for language in [Language::English, Language::German] {
                            ui.selectable_value(
                                &mut self.config.language,
                                Some(language),
                                language_name(language),
                            );
                        }
                    });
                ui.end_row();
            });

        ui.separator();
        ui.label(egui::RichText::new(tr(Text::BorderlessHint)).weak());
        ui.label(egui::RichText::new(tr(Text::MinimizeHint)).weak());
        ui.label(
            egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                .weak()
                .small(),
        );
    }

    fn levels(&mut self, ui: &mut egui::Ui, lang: Language) {
        let can_remove = self.config.levels.len() > 1;
        let mut remove = None;
        for (i, level) in self.config.levels.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(level, MIN_FACTOR..=MAX_FACTOR)
                        .step_by(0.5)
                        .suffix("×"),
                );
                if can_remove && ui.small_button(text(lang, Text::RemoveLevel)).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.config.levels.remove(i);
        }
        if self.config.levels.len() < MAX_LEVELS && ui.button(text(lang, Text::AddLevel)).clicked()
        {
            let next = self.config.levels.last().map_or(2.0, |last| last * 2.0);
            self.config.levels.push(next.clamp(MIN_FACTOR, MAX_FACTOR));
        }
    }
}

impl eframe::App for MainWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_events(&ctx);
        egui::CentralPanel::default().show(ui, |ui| self.settings(ui));
        self.sync(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}

/// German if Windows is set to German, English otherwise.
fn system_language() -> Language {
    // SAFETY: plain query without arguments.
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    if lang_id & 0x3FF == LANG_GERMAN {
        Language::German
    } else {
        Language::English
    }
}
