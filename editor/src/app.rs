//! Coquille egui : rail 8 modes (v3-hifi), tête 36px, grille Build, viewport / inspecteur.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use eframe::egui::{Align2, RichText};

use w3drs_assistant_api::AssistantConfig;
use w3drs_assistant_api::AssistantError;
use w3drs_assistant_api::AssistantEvent;
use w3drs_assistant_api::NoopBackend;

use crate::assistant::{drain_assistant_events, make_assistant_backend, try_load_assistant_config};
use crate::editor_config::{parse_editor_config_str, Appearance, EditorUi};
use crate::mode_rail::{draw_mode_button, paint_rail_brand};
use crate::motor::{engine_status_line, load_engine_bootstrap, EngineBootstrap};
use crate::v3_hifi::{self, c, D_LINE};
use crate::viewport3d::Viewport3dPbr;

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
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "w3d — editor (natif)",
        native_options,
        Box::new({
            let l = launch;
            move |cc| {
                v3_hifi::apply_egui_visuals(&cc.egui_ctx, l.editor.shell.appearance);
                Ok(Box::new(EditorShellApp::new(l)) as Box<dyn eframe::App>)
            }
        }),
    )
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
    /// Texture rail (`w3d_logo.svg`), ou repli [mode_rail::paint_rail_brand].
    w3d_rail_brand: Option<egui::TextureHandle>,
    w3d_rail_brand_try_done: bool,
    /// Clic sur le FAB : focus prompt assistant au prochain `inspector_content`.
    asst_request_focus: bool,
    /// Viewport 3D PBR (même pipeline que `khronos-pbr-sample`, via `khronos_pbr`).
    viewport_pbr: Viewport3dPbr,
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
            w3d_rail_brand: None,
            w3d_rail_brand_try_done: false,
            asst_request_focus: false,
            viewport_pbr: Viewport3dPbr::new(),
        }
    }

    fn play_mode(&self) -> bool {
        self.active == 6
    }

    fn build_mode(&self) -> bool {
        self.active == 0
    }

    /// Une tentative : [crate::w3d_logo] ou repli peint.
    fn ensure_w3d_rail_brand(&mut self, ctx: &egui::Context) {
        if self.w3d_rail_brand_try_done {
            return;
        }
        self.w3d_rail_brand_try_done = true;
        if let Some(img) = crate::w3d_logo::rasterize_w3d_brand_image(32) {
            self.w3d_rail_brand = Some(
                ctx.load_texture("w3d_rail_brand", img, egui::TextureOptions::LINEAR),
            );
        }
    }

    fn apply_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        ctx.input(|i| {
            if i.key_pressed(Key::B) || (i.key_pressed(Key::Escape) && self.active == 6) {
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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
        self.ensure_w3d_rail_brand(ctx);

        let w = self.config.shell.layout.rail_width_css_px.max(44) as f32;
        let ap = self.config.shell.appearance;
        let dark = matches!(ap, Appearance::Dark);
        let d = v3_hifi::color_dark();
        let l = v3_hifi::color_light();
        let (rail_bg, _rail_stroke, _play_rail) = v3_hifi::rail_skin(self.play_mode());
        let (head_bg, title_c) = v3_hifi::head_skin(self.play_mode());

        let play = self.play_mode();
        let build = self.build_mode();

        let title = self.config.stage.title.clone();
        let active = self.active;
        let lab = self.config.modes[active].label.clone();
        let crumb = format!("w3d · {lab} · pbr sample");

        let modes: [crate::editor_config::ModeEntry; 8] = self.config.modes.clone();

        let line_stroke = if dark { c(D_LINE) } else { l.line };
        egui::SidePanel::left("rail")
            .exact_width(w)
            .resizable(false)
            .frame(
                egui::Frame::default()
                    .fill(rail_bg)
                    .inner_margin(4.0)
                    .stroke(egui::Stroke::new(1.0, line_stroke)),
            )
            .show(ctx, |ui| {
                let (brand_rect, _) = ui.allocate_at_least(egui::vec2(32.0, 32.0), egui::Sense::hover());
                if ui.is_rect_visible(brand_rect) {
                    if let Some(ref tex) = self.w3d_rail_brand {
                        let _r = ui.put(
                            brand_rect,
                            egui::Image::from_texture((tex.id(), tex.size_vec2())).corner_radius(6.0),
                        );
                    } else {
                        paint_rail_brand(
                            &ui.painter().with_clip_rect(brand_rect),
                            brand_rect,
                            self.play_mode(),
                        );
                    }
                }
                ui.add_space(6.0);
                for (i, m) in modes.iter().enumerate() {
                    let sel = i == active;
                    let r = draw_mode_button(ui, i, m, sel, play);
                    if r.clicked() {
                        self.active = i;
                    }
                }
            });

        egui::SidePanel::right("inspector")
            .default_width(300.0)
            .resizable(true)
            .frame(
                egui::Frame::default()
                    .fill(if dark { d.bg_panel } else { l.selection })
                    .inner_margin(8.0)
                    .stroke(egui::Stroke::new(1.0, if dark { d.line } else { l.line })),
            )
            .show(ctx, |ui| {
                self.inspector_content(ui, dark, &d, &l);
            });

        let head_h = 40.0_f32;
        let status_motor = engine_status_line(&self.engine);
        let src = self.engine.source_path.display().to_string();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(if dark { d.bg_app } else { l.bg_app }),
            )
            .show(ctx, |ui| {
                // Tête d’espace (≈ 40px) — *Play* : teinte head_skin (dégradé → fill unique).
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), head_h),
                    egui::Layout::left_to_right(egui::Align::Center)
                        .with_main_align(egui::Align::Min)
                        .with_main_justify(false),
                    |ui| {
                        let r = ui.max_rect();
                        let line_c = if dark { c(D_LINE) } else { l.line };
                        let p = ui.painter();
                        p.rect_filled(r, 0.0, head_bg);
                        p.hline(
                            r.x_range(),
                            r.max.y,
                            egui::Stroke::new(1.0, line_c),
                        );
                        ui.set_min_width(r.width());
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(&title)
                                .strong()
                                .size(13.0)
                                .color(title_c),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(&crumb)
                                .size(12.0)
                                .color(if play {
                                    c([0x6b, 0x63, 0x59])
                                } else if dark {
                                    d.fg_soft
                                } else {
                                    l.fg_soft
                                }),
                        );
                    },
                );
                if build {
                    self.draw_build_view(ctx, frame, ui, dark, &d, &l, &status_motor, &src);
                } else {
                    self.draw_nonbuild_placeholder(
                        ctx,
                        frame,
                        ui,
                        dark,
                        &d,
                        &l,
                        lab.as_str(),
                        &status_motor,
                        src.as_str(),
                        play,
                    );
                }
            });

        if build {
            let fab_fill = if dark { c(v3_hifi::D_BRAND) } else { l.brand };
            let fab_txt = c(v3_hifi::D_BRAND_FG);
            egui::Area::new(egui::Id::new("w3d_fab"))
                .order(egui::Order::Foreground)
                .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -52.0))
                .show(ctx, |ui| {
                    if ui
                        .add_sized(
                            [40.0, 40.0],
                            egui::Button::new(RichText::new("✦").size(18.0).color(fab_txt))
                                .fill(fab_fill),
                        )
                        .on_hover_text("Assistant — focus saisie (panneau droit)")
                        .clicked()
                    {
                        self.asst_request_focus = true;
                    }
                });
        }
    }
}

impl EditorShellApp {
    fn draw_build_view(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        ui: &mut egui::Ui,
        dark: bool,
        d: &v3_hifi::HifiDark,
        l: &v3_hifi::HifiLight,
        status_motor: &str,
        src: &str,
    ) {
        ui.horizontal_top(|ui| {
            // Outliner (maquette ~180px)
            let ol_w = 180.0;
            let fill = if dark { d.bg_panel } else { l.selection };
            ui.vertical(|ui| {
                ui.set_max_width(ol_w);
                ui.set_min_width(ol_w);
                egui::Frame::default()
                    .fill(fill)
                    .inner_margin(6.0)
                    .stroke(egui::Stroke::new(1.0, if dark { d.line } else { l.line }))
                    .show(ui, |ui| {
                        ui.heading(
                            RichText::new("OUTLINER")
                                .size(10.0)
                                .color(if dark { d.fg_soft } else { l.fg_soft })
                                .family(egui::FontFamily::Monospace),
                        );
                        ui.add_space(4.0);
                        ui.label(RichText::new("Jalon : reflet ECS (Web → natif)").color(
                            if dark { d.fg_2 } else { l.fg },
                        ));
                        for row in [
                            "Scene (placeholder)",
                            "  Active Camera",
                            "  Primitives modèle",
                            "  Backdrop / Ground",
                        ] {
                            ui.add_space(2.0);
                            if row.starts_with("  ") {
                                let _r = ui.selectable_label(
                                    false,
                                    RichText::new(row)
                                        .size(12.0)
                                        .color(if dark { d.fg_2 } else { l.fg }),
                                );
                            } else {
                                let _r = ui.selectable_label(
                                    true,
                                    RichText::new(row)
                                        .size(10.0)
                                        .strong()
                                        .color(if dark { d.fg_soft } else { l.fg_soft })
                                        .family(egui::FontFamily::Monospace),
                                );
                            }
                        }
                    });
            });
            // Viewport (reste de la ligne)
            self.viewport_central(
                ctx,
                frame,
                ui,
                dark,
                d,
                l,
                if dark { d.bg_viewport } else { c([0xec, 0xe7, 0xdf]) },
                status_motor,
                src,
                true,
            );
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_nonbuild_placeholder(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        ui: &mut egui::Ui,
        dark: bool,
        d: &v3_hifi::HifiDark,
        l: &v3_hifi::HifiLight,
        lab: &str,
        status_motor: &str,
        src: &str,
        _play: bool,
    ) {
        let vfill = if dark { d.bg_viewport } else { c([0xec, 0xe7, 0xdf]) };
        let rect = ui.max_rect();
        if self.play_mode() {
            let hud = rect.with_min_y(rect.min.y + 8.0);
            let hud = hud.shrink2(egui::vec2(rect.width() * 0.15, 0.0));
            let _hud = hud.shrink(4.0);
            let y = egui::Rect::from_min_size(
                egui::pos2(hud.center().x - 100.0, hud.min.y),
                egui::vec2(200.0, 24.0),
            );
            ui.painter().rect_filled(
                y,
                20.0,
                egui::Color32::from_rgba_premultiplied(12, 10, 8, 150),
            );
            ui.painter()
                .text(
                    y.center(),
                    egui::Align2::CENTER_CENTER,
                    "Lecture · Esc → retour Build",
                    egui::FontId::monospace(10.0),
                    if dark { d.fg } else { l.fg },
                );
        }
        self.viewport_central(
            ctx,
            frame,
            ui,
            dark,
            d,
            l,
            vfill,
            status_motor,
            &format!("{src} — mode {lab}"),
            false,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn viewport_central(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        ui: &mut egui::Ui,
        dark: bool,
        d: &v3_hifi::HifiDark,
        l: &v3_hifi::HifiLight,
        fill: egui::Color32,
        status_motor: &str,
        sub: &str,
        try_pbr: bool,
    ) {
        let avail = ui.available_size();
        let (_id, vrect) = ui.allocate_space(avail);
        if !ui.is_rect_visible(vrect) {
            return;
        }
        if try_pbr && self.viewport_pbr.show(ui, ctx, frame, vrect) {
            let p = ui.painter();
            p.text(
                vrect.max - egui::vec2(6.0, 6.0 + 12.0),
                egui::Align2::RIGHT_BOTTOM,
                "Viewport PBR (Khronos) · souris : orbite, molette : zoom",
                egui::FontId::proportional(9.0),
                if dark { d.fg_faint } else { l.fg_soft },
            );
            p.text(
                vrect.max - egui::vec2(6.0, 6.0 + 9.0 + 14.0),
                egui::Align2::RIGHT_BOTTOM,
                status_motor,
                egui::FontId::monospace(8.0),
                if dark { d.fg_2 } else { l.fg_soft },
            );
            p.text(
                vrect.max - egui::vec2(6.0, 6.0 + 9.0 + 14.0 * 2.0),
                egui::Align2::RIGHT_BOTTOM,
                sub,
                egui::FontId::monospace(8.0),
                if dark { d.fg_2 } else { l.fg_soft },
            );
            return;
        }
        let p = ui.painter();
        p.rect_filled(vrect, 6.0, fill);
        p.rect_stroke(
            vrect,
            6.0,
            egui::epaint::Stroke::new(1.0, if dark { d.line } else { l.line }),
            egui::StrokeKind::Inside,
        );
        p.text(
            vrect.center() - egui::vec2(0.0, 24.0),
            egui::Align2::CENTER_CENTER,
            "Viewport 3D — wgpu requis (eframe wgpu) ou pilote / surface indisponible",
            egui::FontId::proportional(12.0),
            if dark { d.fg_soft } else { l.fg },
        );
        p.text(
            vrect.center(),
            egui::Align2::CENTER_CENTER,
            status_motor,
            egui::FontId::proportional(9.0),
            if dark { d.fg_2 } else { l.fg_soft },
        );
        p.text(
            vrect.center() + egui::vec2(0.0, 20.0),
            egui::Align2::CENTER_CENTER,
            sub,
            egui::FontId::monospace(10.0),
            if dark { d.fg_faint } else { l.fg_soft },
        );
    }

    fn inspector_content(
        &mut self,
        ui: &mut egui::Ui,
        dark: bool,
        d: &v3_hifi::HifiDark,
        l: &v3_hifi::HifiLight,
    ) {
        use crate::assistant::build_user_completion;
        let enabled = self.assistant_config.enabled;
        let ollish = self.assistant_config.base_url.as_str();
        let model = self.assistant_config.model.as_str();
        let bnote = self.backend_note.as_str();
        let muted2 = if dark { d.fg_faint } else { l.fg_soft };

        ui.heading(
            RichText::new("Inspector")
                .size(12.0)
                .color(if dark { d.fg_strong } else { l.fg }),
        );
        ui.label(
            RichText::new("Réglages moteur / entités (placeholder).")
                .italics()
                .size(12.0)
                .color(if dark { d.fg_soft } else { l.fg_soft }),
        );
        ui.add_space(10.0);
        ui.heading(
            RichText::new("Assistant (optionnel)")
                .size(12.0)
                .color(if dark { d.fg_2 } else { l.fg }),
        );
        ui.label(RichText::new(bnote).small().color(muted2));
        ui.add_space(4.0);
        if enabled {
            ui.label(
                RichText::new(format!("Oui · model={model} · {ollish}"))
                    .small()
                    .color(if dark { d.fg_2 } else { l.fg }),
            );
        } else {
            ui.label(
                RichText::new("Off (enabled=false)")
                    .small()
                    .color(muted2),
            );
        }
        ui.add_space(6.0);
        let w_in = (ui.available_width() - 4.0).max(32.0);
        let prompt = ui.add(
            egui::TextEdit::multiline(&mut self.asst_prompt)
                .desired_width(w_in)
                .min_size(egui::vec2(0.0, 56.0))
                .hint_text("Saisie message (V1) — jamais écrit sur disque sans toi…"),
        );
        if self.asst_request_focus {
            prompt.request_focus();
            self.asst_request_focus = false;
        }
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
        ui.label(
            RichText::new("Sortie (flux) :")
                .small()
                .color(if dark { d.fg_soft } else { l.fg }),
        );
        let _o = ui.add(
            egui::TextEdit::multiline(&mut self.asst_out)
                .desired_width(w_in)
                .min_size(egui::vec2(0.0, 100.0))
                .interactive(false),
        );
    }
}
