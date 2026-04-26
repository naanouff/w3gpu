//! Coquille egui : rail 8 modes, tête, viewport / inspecteur (placeholders + assistant **optionnel**).

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use eframe::egui::{Key, RichText};

use w3drs_assistant_api::AssistantConfig;
use w3drs_assistant_api::AssistantError;
use w3drs_assistant_api::AssistantEvent;
use w3drs_assistant_api::NoopBackend;

use crate::assistant::{build_user_completion, drain_assistant_events, make_assistant_backend, try_load_assistant_config};
use crate::editor_config::{parse_editor_config_str, Appearance, EditorUi};
use crate::motor::{engine_status_line, load_engine_bootstrap, EngineBootstrap};

/// Tout le nécessaire pour un lancement personnalisé (écrans d’essai, tests, CI).
pub struct EditorLaunch {
    pub editor: EditorUi,
    pub assistant_config: AssistantConfig,
    pub backend: Arc<dyn w3drs_assistant_api::AssistantBackend + Send + Sync>,
    /// Court message pour l’UI (Ollama / Noop / feature manquante).
    pub backend_note: String,
}

/// `editor/` → parent = racine du dépôt.
fn default_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("phases")
        .join("phase-k")
        .join("editor-ui.json")
}

/// Charge `editor-ui.json` (chemin par défaut ou `--config`), `assistant.json` côtier si présent, lance.
pub fn run() -> eframe::Result {
    let mut path = default_config_path();
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--config" {
            if let Some(p) = args.get(i + 1) {
                path = PathBuf::from(p);
            }
            break;
        }
    }

    let text = if path.is_file() {
        std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("Lecture {}: {e}", path.display());
            std::process::exit(1);
        })
    } else {
        eprintln!("Fichier config introuvable ({}), fallback binaire embarqué.", path.display());
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/phases/phase-k/editor-ui.json"))
            .to_string()
    };
    let config = parse_editor_config_str(&text).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let a_cfg = try_load_assistant_config(&path);
    let (b, note) = make_assistant_backend(&a_cfg);
    run_with_launch(EditorLaunch {
        editor: config,
        assistant_config: a_cfg,
        backend: b,
        backend_note: note,
    })
}

/// Fenêtre native (tests : injecter un `EditorUi` + assistant en Noop / défaut).
pub fn run_with_config(config: EditorUi) -> eframe::Result {
    run_with_launch(EditorLaunch {
        editor: config,
        assistant_config: AssistantConfig::default(),
        backend: Arc::new(NoopBackend),
        backend_note: "Noop (défaut, pas de assistant.json)".to_string(),
    })
}

/// Lancement complet (assistant, backend).
pub fn run_with_launch(launch: EditorLaunch) -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("w3d — editor (natif)"),
        ..Default::default()
    };
    eframe::run_native(
        "w3d — editor (natif)",
        native_options,
        Box::new({
            let l = launch;
            move |cc| {
                apply_theme(&cc.egui_ctx, l.editor.shell.appearance);
                Ok(Box::new(EditorShellApp::new(l)) as Box<dyn eframe::App>)
            }
        }),
    )
}

fn apply_theme(ctx: &egui::Context, appearance: Appearance) {
    let mut v = match appearance {
        Appearance::Dark => egui::Visuals::dark(),
        Appearance::Light => egui::Visuals::light(),
    };
    v.panel_fill = egui::Color32::from_rgb(0x1f, 0x1d, 0x1a);
    v.window_fill = egui::Color32::from_rgb(0x16, 0x15, 0x13);
    v.extreme_bg_color = egui::Color32::from_rgb(0x10, 0x0f, 0x0d);
    v.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(0x2a, 0x27, 0x24);
    v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(0x3a, 0x2d, 0x22);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(0xd9, 0x77, 0x57);
    v.selection.bg_fill = egui::Color32::from_rgb(0x58, 0x2a, 0x1c);
    v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0xe0, 0x85, 0x62));
    if matches!(appearance, Appearance::Light) {
        v.panel_fill = egui::Color32::from_rgb(0xfa, 0xf8, 0xf5);
        v.window_fill = egui::Color32::from_rgb(0xfa, 0xf8, 0xf5);
        v.extreme_bg_color = egui::Color32::from_rgb(0xf0, 0xeb, 0xe1);
    }
    ctx.set_visuals(v);
}

struct EditorShellApp {
    config: EditorUi,
    active: usize,
    engine: EngineBootstrap,
    assistant_config: AssistantConfig,
    assistant_backend: Arc<dyn w3drs_assistant_api::AssistantBackend + Send + Sync>,
    backend_note: String,
    asst_prompt: String,
    asst_out: String,
    asst_stream: Option<w3drs_assistant_api::ActiveStream>,
}

impl EditorShellApp {
    fn new(l: EditorLaunch) -> Self {
        Self {
            config: l.editor,
            active: 0,
            engine: load_engine_bootstrap(),
            assistant_config: l.assistant_config,
            assistant_backend: l.backend,
            backend_note: l.backend_note,
            asst_prompt: String::new(),
            asst_out: String::new(),
            asst_stream: None,
        }
    }

    fn apply_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(Key::B) {
                self.active = 0;
            } else if i.key_pressed(Key::P) {
                self.active = 1;
            } else if i.key_pressed(Key::S) {
                self.active = 2;
            } else if i.key_pressed(Key::L) {
                self.active = 3;
            } else if i.key_pressed(Key::A) {
                self.active = 4;
            } else if i.key_pressed(Key::I) {
                self.active = 5;
            } else if i.key_pressed(Key::Space) {
                self.active = 6;
            }
        });
    }
}

impl eframe::App for EditorShellApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(ref s) = self.asst_stream {
            let (evs, _err) = drain_assistant_events(s);
            let mut done = false;
            for e in evs {
                match e {
                    AssistantEvent::Token(t) => {
                        self.asst_out.push_str(&t);
                    }
                    AssistantEvent::Done => done = true,
                    AssistantEvent::Error(e) => {
                        if !matches!(&e, AssistantError::Aborted) {
                            self.asst_out = format!("Erreur: {e}");
                        }
                        done = true;
                    }
                }
            }
            if done {
                self.asst_stream = None;
            }
        }
        self.apply_shortcuts(ctx);
        let w = self.config.shell.layout.rail_width_css_px as f32;
        let title = &self.config.stage.title;
        let active = self.active;
        let lab = self.config.modes[active].label.as_str();
        let crumb = format!("w3d · {lab} · pbr sample");

        egui::TopBottomPanel::top("head").show(ctx, |ui| {
            ui.heading(
                RichText::new(title)
                    .strong()
                    .size(18.0)
                    .color(egui::Color32::from_rgb(0xf0, 0xe8, 0xdc)),
            );
            ui.label(
                RichText::new(&crumb)
                    .small()
                    .color(egui::Color32::from_rgb(0x8b, 0x83, 0x77)),
            );
        });

        let modes: &[crate::editor_config::ModeEntry; 8] = &self.config.modes;
        egui::SidePanel::left("rail")
            .exact_width(w)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("w3d")
                        .strong()
                        .size(14.0)
                        .color(rail_accent()),
                );
                ui.add_space(8.0);
                for (i, m) in modes.iter().enumerate() {
                    let sel = i == active;
                    let line = if m.key_hint.is_empty() {
                        m.label.clone()
                    } else {
                        format!("{}\n{}", m.label, m.key_hint)
                    };
                    if ui
                        .selectable_label(
                            sel,
                            RichText::new(line)
                                .small()
                                .color(if sel { rail_on() } else { rail_off() }),
                        )
                        .clicked()
                    {
                        self.active = i;
                    }
                }
            });

        egui::SidePanel::right("inspector")
            .default_width(300.0)
            .resizable(true)
            .show(ctx, |ui| {
                let enabled = self.assistant_config.enabled;
                let ollish = self.assistant_config.base_url.as_str();
                let model = self.assistant_config.model.as_str();
                let bnote = self.backend_note.as_str();

                ui.heading(RichText::new("Inspector").color(muted()));
                ui.label(
                    RichText::new("Placeholder : réglages / props (moteur plus tard).")
                        .italics()
                        .color(muted2()),
                );
                ui.add_space(10.0);
                ui.heading(
                    RichText::new("Assistant (optionnel)")
                        .size(15.0)
                        .color(muted()),
                );
                ui.label(RichText::new(bnote).small().color(muted2()));
                ui.add_space(4.0);
                if enabled {
                    ui.label(
                        RichText::new(format!("Oui · model={model} · {ollish}"))
                            .small()
                            .color(muted()),
                    );
                } else {
                    ui.label(
                        RichText::new("Off (enabled=false)")
                            .small()
                            .color(muted2()),
                    );
                }
                ui.add_space(6.0);
                let w_in = (ui.available_width() - 4.0).max(32.0);
                let _p = ui.add(
                    egui::TextEdit::multiline(&mut self.asst_prompt)
                        .desired_width(w_in)
                        .min_size(egui::vec2(0.0, 56.0))
                        .hint_text("Saisie message (V1) — jamais écrit sur disque sans toi…"),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            enabled,
                            egui::Button::new("Envoyer (HTTP si sidecar)"),
                        )
                        .on_hover_text("Requiert `enabled: true` dans assistant.json. Sidecar: build avec --features sidecar-ollama")
                        .clicked()
                    {
                        if let Some(s) = self.asst_stream.take() {
                            s.cancel();
                            let _ = s.join();
                        }
                        if !self.asst_prompt.is_empty() {
                            let r = build_user_completion(None, self.asst_prompt.as_str());
                            self.asst_out.clear();
                            match self.assistant_backend.start_completion(r) {
                                Ok(st) => self.asst_stream = Some(st),
                                Err(e) => self.asst_out = format!("Démarrage: {e}"),
                            }
                        } else {
                            self.asst_out = "Saisis un message d’abord.".to_string();
                        }
                    }
                    if ui
                        .add_enabled(
                            self.asst_stream.is_some() && enabled,
                            egui::Button::new("Annuler"),
                        )
                        .on_hover_text("Annule la requête en cours")
                        .clicked()
                    {
                        if let Some(s) = self.asst_stream.as_ref() {
                            s.cancel();
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Sortie (flux) :").small().color(muted()));
                let _o = ui.add(
                    egui::TextEdit::multiline(&mut self.asst_out)
                        .desired_width(w_in)
                        .min_size(egui::vec2(0.0, 100.0))
                        .interactive(false),
                );
            });

        let status_motor = engine_status_line(&self.engine);
        let src = self.engine.source_path.display().to_string();
        egui::CentralPanel::default().show(ctx, |ui| {
            let h = ui.available_height();
            let rect = ui.max_rect();
            ui.painter()
                .rect_filled(rect, 3.0, egui::Color32::from_rgb(0x0f, 0x0e, 0x0c));
            ui.add_space((h * 0.3).max(0.0));
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Viewport 3D — prochaine étape : eframe wgpu + surface w3drs (même wgpu qu’egui)").color(muted()),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(&status_motor).small().color(muted2()));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Fichier : {src}"))
                        .small()
                        .italics()
                        .color(muted2()),
                );
            });
        });
    }
}

fn rail_accent() -> egui::Color32 {
    egui::Color32::from_rgb(0xe0, 0x85, 0x62)
}

fn rail_on() -> egui::Color32 {
    egui::Color32::from_rgb(0xfa, 0xf0, 0xe6)
}

fn rail_off() -> egui::Color32 {
    egui::Color32::from_rgb(0xa5, 0x9c, 0x8c)
}

fn muted() -> egui::Color32 {
    egui::Color32::from_rgb(0xc5, 0xbf, 0xb1)
}

fn muted2() -> egui::Color32 {
    egui::Color32::from_rgb(0x7d, 0x75, 0x69)
}
