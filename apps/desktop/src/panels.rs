//! The window's layout: top bar, bottom bar, the tabbed side panel and the map.

use crate::map::{self, Palette, Xf};
use crate::params_ui::field_row;
use crate::pick::map_object_at;
use crate::{
    class_label, format_number, get_path, set_path, size_description, size_label, summary,
    translate_log, translate_violation, App, BenchmarkKind, Experiment, GeneratedMap, Tab,
};
use eframe::egui::{
    self, Align, Button, Color32, CursorIcon, Frame, Grid, Layout, Panel, RichText, ScrollArea,
    Sense, Stroke, TextEdit, Ui, Vec2,
};
use serde_json::{json, Value};
use tapiti_core::par;
use tapiti_core::params::Size;

fn muted(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .small()
        .color(ui.visuals().weak_text_color())
}

fn card(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    Frame::group(ui.style()).inner_margin(12.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(15.0));
        ui.add_space(2.0);
        body(ui);
    });
    ui.add_space(4.0);
}

fn primary(ui: &Ui, text: &str) -> Button<'static> {
    Button::new(
        RichText::new(text.to_string())
            .strong()
            .color(Color32::WHITE),
    )
    .fill(ui.visuals().selection.bg_fill)
}

pub fn top_bar(app: &mut App, ui: &mut Ui) {
    Panel::top("top").show(ui, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("Tapiti");
            ui.separator();
            ui.label("Size");
            for (n, size) in Size::ALL.into_iter().enumerate() {
                let hint = format!("{}\nAlt+{}", size_description(size), n + 1);
                if ui
                    .selectable_label(app.size == size, size_label(size))
                    .on_hover_text(hint)
                    .clicked()
                {
                    app.set_size(size);
                }
            }
            ui.separator();
            ui.label("Seed");
            if ui
                .button("<")
                .on_hover_text("Previous seed (Alt+Left)")
                .clicked()
            {
                app.set_seed(app.seed.saturating_sub(1));
            }
            let seed = ui.add(
                TextEdit::singleline(&mut app.seed_draft)
                    .desired_width(92.0)
                    .hint_text("0–4294967295"),
            );
            if seed.lost_focus() && app.seed_draft != app.seed.to_string() {
                app.commit_seed_draft();
            }
            if ui
                .button(">")
                .on_hover_text("Next seed (Alt+Right)")
                .clicked()
            {
                app.set_seed(app.seed.saturating_add(1));
            }
            if ui
                .button("Random")
                .on_hover_text("New random seed (Alt+R)")
                .clicked()
            {
                app.random_seed();
            }
            ui.separator();
            if ui
                .add(primary(ui, "Generate"))
                .on_hover_text("Generate again with the current Λ")
                .clicked()
            {
                app.generate_now();
            }
            if app.busy {
                ui.spinner();
            }
        });
        ui.add_space(4.0);
    });
}

pub fn bottom_bar(app: &mut App, ui: &mut Ui) {
    Panel::bottom("bottom").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(muted(ui, &app.status));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(muted(
                    ui,
                    format!(
                        "Rust engine · Rayon · {threads} threads",
                        threads = par::threads()
                    ),
                ));
                ui.separator();
                let mut theme = ui.options(|o| o.theme_preference);
                for (value, name) in [
                    (egui::ThemePreference::Dark, "Dark"),
                    (egui::ThemePreference::Light, "Light"),
                    (egui::ThemePreference::System, "Auto"),
                ] {
                    if ui.selectable_label(theme == value, name).clicked() {
                        theme = value;
                    }
                }
                ui.ctx().set_theme(theme);
            });
        });
    });
}

pub fn side_panel(app: &mut App, ui: &mut Ui) {
    Panel::right("side")
        .default_size(380.0)
        .size_range(320.0..=560.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let params = if app.changed.is_empty() {
                    "Parameters".to_string()
                } else {
                    format!("Parameters ({n})", n = app.changed.len())
                };
                for (tab, name) in [
                    (Tab::Floor, "Floor".to_string()),
                    (Tab::Parameters, params),
                    (Tab::Analysis, "Analysis".into()),
                ] {
                    if ui.selectable_label(app.tab == tab, name).clicked() {
                        app.tab = tab;
                    }
                }
            });
            ui.separator();
            ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| match app.tab {
                    Tab::Floor => floor_tab(app, ui),
                    Tab::Parameters => parameters_tab(app, ui),
                    Tab::Analysis => analysis_tab(app, ui),
                });
        });
}

// ---- Andar ---------------------------------------------------------------------------------------

fn verdict(ui: &mut Ui, map: &GeneratedMap) {
    let pal = Palette::new(ui.visuals().dark_mode);
    if map.violations.is_empty() {
        ui.label(
            RichText::new("Valid map")
                .strong()
                .size(16.0)
                .color(pal.success),
        );
    } else {
        ui.label(
            RichText::new(format!("! {n} problem(s)", n = map.violations.len()))
                .strong()
                .size(16.0)
                .color(pal.danger),
        );
    }
}

fn floor_tab(app: &mut App, ui: &mut Ui) {
    card(ui, "Reading", |ui| match &app.generated {
        None => {
            ui.horizontal(|ui| {
                if app.busy {
                    ui.spinner();
                }
                ui.label(if app.busy {
                    "Building the floor…"
                } else {
                    "No floor generated."
                });
            });
        }
        Some(map) => {
            verdict(ui, map);
            let m = &map.metrics;
            Grid::new("reading")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (label, value) in [
                        ("Rooms", m.rooms.to_string()),
                        ("Corridors", m.edges.to_string()),
                        (
                            "Cycles β",
                            format!(
                                "{beta} (target {target})",
                                beta = m.beta,
                                target = m.beta_target
                            ),
                        ),
                        (
                            "Routes to the exit",
                            m.routes_to_exit.map_or("—".into(), |n| n.to_string()),
                        ),
                        ("Attempts", map.draws.to_string()),
                        ("Generation", format!("{:.1} ms", map.elapsed_ms)),
                    ] {
                        ui.label(muted(ui, label));
                        ui.label(value);
                        ui.end_row();
                    }
                });
            for violation in map.violations.iter().take(6) {
                ui.label(muted(ui, translate_violation(violation)));
            }
            if map.violations.len() > 6 {
                ui.label(muted(
                    ui,
                    format!(
                        "… and {n} more. See the Analysis tab.",
                        n = map.violations.len() - 6
                    ),
                ));
            }
        }
    });

    card(ui, "Layout", |ui| {
        let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
        let mut edit = None;
        Grid::new("layout")
            .num_columns(3)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                if let Some(field) = app
                    .groups
                    .iter()
                    .flat_map(|g| &g.fields)
                    .find(|f| f.path == "layout")
                {
                    let value = get_path(&current, "layout").cloned().unwrap_or(Value::Null);
                    if let Some(next) =
                        field_row(ui, field, &value, get_path(&app.defaults, "layout"))
                    {
                        edit = Some((field.path.clone(), next));
                    }
                }
            });
        if let Some((path, value)) = edit {
            app.change_param(&path, value);
        }
        if app.params.layout == tapiti_core::params::Layout::Growth {
            let mut edit = None;
            Grid::new("growth")
                .num_columns(3)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for field in app.groups.iter().flat_map(|g| &g.fields).filter(|f| {
                        [
                            "growth.rooms",
                            "growth.islands",
                            "growth.branch",
                            "growth.depth",
                            "growth.cluster",
                            "growth.reach",
                        ]
                        .contains(&f.path.as_str())
                    }) {
                        let value = get_path(&current, &field.path)
                            .cloned()
                            .unwrap_or(Value::Null);
                        if let Some(next) =
                            field_row(ui, field, &value, get_path(&app.defaults, &field.path))
                        {
                            edit = Some((field.path.clone(), next));
                        }
                    }
                });
            if let Some((path, value)) = edit {
                app.change_param(&path, value);
            }
        } else {
            ui.label(muted(ui, "One room per grid block."));
        }
        ui.label(muted(ui, size_description(app.size)));
    });

    card(ui, "Shortcuts", |ui| {
        for line in [
            "Alt+1–4 switches the size · Alt+R random seed",
            "Alt+Left / Right: previous / next seed",
            "Drag moves the map · wheel zooms at the cursor",
            "Double-click fits the map to the window",
        ] {
            ui.label(muted(ui, line));
        }
    });
}

// ---- Parâmetros ----------------------------------------------------------------------------------

fn parameters_tab(app: &mut App, ui: &mut Ui) {
    ui.label(muted(
        ui,
        "Every change regenerates the map after a short pause. Hover a name to see what it does.",
    ));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut app.filter)
                .hint_text("Filter parameters…")
                .desired_width(ui.available_width() - 34.0),
        );
        if ui
            .add_enabled(!app.filter.is_empty(), Button::new("×"))
            .on_hover_text("Clear filter")
            .clicked()
        {
            app.filter.clear();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Copy JSON")
            .on_hover_text("Size, seed and the full Λ")
            .clicked()
        {
            let export = json!({ "size": size_label(app.size).to_lowercase(), "seed": app.seed, "params": app.params });
            ui.ctx()
                .copy_text(serde_json::to_string_pretty(&export).unwrap_or_default());
            app.status = "Parameters copied as JSON.".into();
        }
        if ui
            .add_enabled(!app.changed.is_empty(), Button::new("Reset to size"))
            .clicked()
        {
            app.set_size(app.size);
        }
        ui.label(muted(
            ui,
            if app.changed.is_empty() {
                "size default".to_string()
            } else {
                format!("{n} changed", n = app.changed.len())
            },
        ));
    });

    ui.add_space(4.0);
    ui.label(RichText::new("Guided experiments").strong());
    ui.horizontal_wrapped(|ui| {
        for (preset, name) in [
            (Experiment::Shapes, "Shapes and diagonals"),
            (Experiment::Cycles, "More routes"),
            (Experiment::Junctions, "T/X junctions"),
            (Experiment::CircularRooms, "Circular rooms"),
        ] {
            if ui.button(name).clicked() {
                app.set_preset(preset);
            }
        }
    });
    ui.add_space(6.0);

    let groups = std::mem::take(&mut app.groups);
    let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
    let needle = app.filter.trim().to_lowercase();
    let mut edits: Vec<(String, Value)> = Vec::new();
    let mut reset = None;
    let mut shown = 0;
    for group in &groups {
        let fields: Vec<_> = group
            .fields
            .iter()
            .filter(|f| {
                needle.is_empty()
                    || f.label.as_str().to_lowercase().contains(&needle)
                    || f.path.to_lowercase().contains(&needle)
            })
            .collect();
        if fields.is_empty() {
            continue;
        }
        shown += 1;
        let changed = group
            .fields
            .iter()
            .filter(|f| app.changed.contains(&f.path))
            .count();
        let title = if changed > 0 {
            format!(
                "{title} · {n} changed",
                title = group.title.as_str(),
                n = changed
            )
        } else {
            group.title.as_str().to_string()
        };
        let header = egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt(&group.id)
            .default_open(group.open_by_default);
        let header = if needle.is_empty() {
            header
        } else {
            header.open(Some(true))
        };
        header.show(ui, |ui| {
            ui.label(muted(ui, group.intro.as_str()));
            Grid::new(&group.id)
                .num_columns(3)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for field in fields {
                        let value = get_path(&current, &field.path)
                            .cloned()
                            .unwrap_or(Value::Null);
                        if let Some(next) =
                            field_row(ui, field, &value, get_path(&app.defaults, &field.path))
                        {
                            edits.push((field.path.clone(), next));
                        }
                    }
                });
            if changed > 0 && ui.small_button("Reset this group").clicked() {
                reset = Some(group.id.clone());
            }
        });
    }
    app.groups = groups;
    if shown == 0 {
        ui.label(muted(ui, "No parameter matches the filter."));
    }
    if let Some(id) = reset {
        app.reset_group(&id);
    } else if !edits.is_empty() {
        let mut value = current;
        for (path, next) in edits {
            set_path(&mut value, &path, next);
        }
        match serde_json::from_value(value) {
            Ok(params) => {
                app.params = params;
                app.params_edited();
            }
            Err(error) => app.status = format!("Invalid parameter: {error}", error = error),
        }
    }

    ui.add_space(8.0);
    ui.label(muted(
        ui,
        format!(
            "Junction conductance: {value} (informational; does not change the generation yet).",
            value = app
                .params
                .junction
                .map(|v| format_number(v, 2))
                .unwrap_or_else(|| "∞, arms joined".into())
        ),
    ));
    ui.add_space(4.0);
    egui::CollapsingHeader::new(RichText::new("Advanced JSON editor").strong())
        .id_salt("json")
        .show(ui, |ui| {
            let editor = ui.add(
                TextEdit::multiline(&mut app.json_text)
                    .code_editor()
                    .desired_rows(16)
                    .desired_width(ui.available_width()),
            );
            if editor.changed() {
                app.json_dirty = true;
                app.apply_at_edit();
            }
            match (&app.json_error, app.json_dirty) {
                (Some(error), _) => {
                    ui.colored_label(Palette::new(ui.visuals().dark_mode).danger, error);
                }
                (None, true) => {
                    ui.label(muted(ui, "Validating before regenerating…"));
                }
                _ => {}
            }
        });
}

// ---- Analysis -------------------------------------------------------------------------------------

fn analysis_tab(app: &mut App, ui: &mut Ui) {
    card(ui, "Performance", |ui| {
        ui.label(muted(
            ui,
            "Benchmarks use the selected size's default parameters.",
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!app.busy, Button::new("Measure 40 maps"))
                .clicked()
            {
                app.start_benchmark(BenchmarkKind::Forty);
            }
            if ui
                .add_enabled(!app.busy, Button::new("Compare batch of 800"))
                .clicked()
            {
                app.start_benchmark(BenchmarkKind::Batch);
            }
        });
        if let Some(text) = &app.performance_status {
            ui.label(text);
        }
    });

    let Some(map) = &app.generated else {
        ui.label(muted(ui, "Metrics and validation appear here."));
        return;
    };
    card(ui, "Floor reading", |ui| {
        ui.label(muted(
            ui,
            format!(
                "{size} size · seed {seed}",
                size = size_label(app.size),
                seed = app.seed
            ),
        ));
        verdict(ui, map);
        if !map.violations.is_empty() {
            ui.label(muted(
                ui,
                format!(
                    "After {n} attempt(s), the map still does not meet every rule.",
                    n = map.draws
                ),
            ));
            for violation in map.violations.iter().take(30) {
                ui.label(muted(ui, translate_violation(violation)));
            }
        }
        ui.label(muted(
            ui,
            format!(
                "{draws} attempt(s) · generation {ms:.1} ms",
                draws = map.draws,
                ms = map.elapsed_ms
            ),
        ));
        ui.label(muted(ui, summary(map)));
    });
    for (title, rows) in metric_sections(map) {
        card(ui, title, |ui| {
            Grid::new(title)
                .num_columns(2)
                .striped(true)
                .min_col_width(ui.available_width() / 2.0 - 12.0)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (label, value, help) in rows {
                        ui.label(label).on_hover_text(help);
                        ui.label(RichText::new(value).strong());
                        ui.end_row();
                    }
                });
        });
    }
    card(ui, "Shapes on the map", |ui| {
        for (class, count) in &map.metrics.by_class {
            ui.label(format!("{}: {count}", class_label(class)));
        }
    });
    if !map.floor.log.is_empty() {
        card(ui, "What the generator did", |ui| {
            for line in &map.floor.log {
                ui.label(muted(ui, translate_log(line)));
            }
        });
    }
}

type Row = (&'static str, String, &'static str);

fn metric_sections(map: &GeneratedMap) -> Vec<(&'static str, Vec<Row>)> {
    let m = &map.metrics;
    let opt = |v: Option<f64>| v.map(|v| format!("{v:.0}")).unwrap_or_else(|| "—".into());
    vec![
        (
            "Map size",
            vec![
                (
                    "Main rooms",
                    m.rooms.to_string(),
                    "Active rooms, not counting leaf chambers.",
                ),
                (
                    "Leaf chambers",
                    m.leaf_chambers.to_string(),
                    "Extra rooms linked by a single corridor.",
                ),
                (
                    "T/X junctions",
                    m.junctions.to_string(),
                    "Points where three or four arms meet.",
                ),
                (
                    "Corridor segments",
                    m.edges.to_string(),
                    "Graph edges; each junction arm counts as a segment.",
                ),
            ],
        ),
        (
            "Cycles and density",
            vec![
                (
                    "Current cycles β",
                    m.beta.to_string(),
                    "Number of independent cycles; a tree has zero.",
                ),
                (
                    "Cycle target",
                    m.beta_target.to_string(),
                    "Target computed from the density and the generator's reference.",
                ),
                (
                    "Reference βmax",
                    m.beta_max.to_string(),
                    "Greedy estimate of the cycles that fit, including junctions.",
                ),
                (
                    "Density δ",
                    format!("{:.2}", m.delta),
                    "Ratio between current cycles and the βmax reference.",
                ),
            ],
        ),
        (
            "Paths and connectivity",
            vec![
                (
                    "Doors used per room",
                    format!("{:.2}", m.mean_degree),
                    "Average number of corridors linked to each room.",
                ),
                (
                    "Single-door rooms",
                    m.leaves.to_string(),
                    "Rooms with only one connection; includes dead ends, the entrance and the exit.",
                ),
                (
                    "Bridges",
                    format!("{} · {:.0}%", m.bridges, m.bridge_share * 100.0),
                    "Segments whose removal would disconnect part of the map.",
                ),
                (
                    "Articulation rooms",
                    m.articulations.to_string(),
                    "Rooms whose removal would split the graph.",
                ),
                (
                    "Steps to the exit",
                    opt(m.hops_to_exit),
                    "Number of rooms on the shortest path from the entrance to the exit.",
                ),
                (
                    "Diameter",
                    format!("{:.0}", m.diameter),
                    "Largest minimum distance between two rooms, in steps.",
                ),
                (
                    "Independent routes to the exit",
                    m.routes_to_exit.map_or("—".into(), |n| n.to_string()),
                    "Paths between the entrance and the exit that share no corridors.",
                ),
                (
                    "Shortest cycle",
                    m.cycle_girth
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "—".into()),
                    "Number of segments in the map's shortest cycle.",
                ),
                (
                    "Side branches",
                    m.side_branches.to_string(),
                    "Optional components attached to the main path by a single passage.",
                ),
                (
                    "Branch depth",
                    m.deepest_branch.to_string(),
                    "Largest number of rooms in a side branch.",
                ),
                (
                    "Highest degree",
                    m.max_node_degree.to_string(),
                    "Largest number of arms joined at one point.",
                ),
                (
                    "Geometric path",
                    m.geometric_path_to_exit
                        .map_or("—".into(), |l| format!("{l:.0} px")),
                    "Physical length of the shortest path to the exit.",
                ),
                (
                    "Connectivity λ₂",
                    format!("{:.3}", m.lambda2),
                    "Larger values tend to indicate less dependence on bottlenecks.",
                ),
            ],
        ),
    ]
}

// ---- Map ----------------------------------------------------------------------------------------

pub fn central(app: &mut App, ui: &mut Ui) {
    egui::CentralPanel::default()
        .frame(Frame::new().inner_margin(10.0))
        .show(ui, |ui| {
            toolbar(app, ui);
            ui.add_space(4.0);
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            let pal = Palette::new(ui.visuals().dark_mode);
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 10.0, pal.background);
            painter.rect_stroke(
                rect,
                10.0,
                Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                egui::StrokeKind::Inside,
            );

            let Some((min, max)) = app.map_bounds() else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Generating the first floor…",
                    egui::FontId::proportional(16.0),
                    pal.ink,
                );
                return;
            };
            let mut xf = Xf::new(rect, min, max, app.zoom, app.pan);
            let ctx = ui.ctx().clone();

            if response.double_clicked() {
                app.zoom = 1.0;
                app.pan = [0.0, 0.0];
            }
            if response.hovered() {
                let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta().y, i.zoom_delta()));
                let factor = f64::from((scroll * 0.0012).exp() * pinch);
                if (factor - 1.0).abs() > 1e-6 {
                    if let Some(pos) = response.hover_pos() {
                        zoom_at(app, &xf, pos, factor);
                        xf = Xf::new(rect, min, max, app.zoom, app.pan);
                    }
                }
            }
            if response.dragged() {
                let delta = response.drag_delta();
                let scale = f64::from(xf.scale).max(1e-6);
                app.pan[0] -= f64::from(delta.x) / scale;
                app.pan[1] -= f64::from(delta.y) / scale;
                xf = Xf::new(rect, min, max, app.zoom, app.pan);
            }
            if response.hovered() {
                ctx.set_cursor_icon(if response.dragged() {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Grab
                });
            }

            // While the map moves the detail layers are dropped, so each frame stays cheap.
            let mut overlays = app.overlays;
            if response.dragged() {
                overlays.labels = false;
                overlays.ports = false;
                overlays.pairs = false;
                overlays.candidates = false;
            }
            if app.showing_building() {
                map::draw_building(&painter, &xf, &app.building, overlays, &pal);
            } else if let Some(generated) = &app.generated {
                map::draw_floor(&painter, &xf, &generated.floor, overlays, &pal);
            }
            if app.overlays.legend && rect.width() > 520.0 && rect.height() > 380.0 {
                map::legend(&painter, rect, &pal, ui.visuals().text_color());
            }
            if app.busy {
                painter.text(
                    rect.right_top() + Vec2::new(-14.0, 16.0),
                    egui::Align2::RIGHT_CENTER,
                    "Generating…",
                    egui::FontId::proportional(13.0),
                    pal.ink.gamma_multiply(0.7),
                );
            }

            if response.hovered() && !response.dragged() && !app.showing_building() {
                if let (Some(pos), Some(generated)) = (response.hover_pos(), &app.generated) {
                    let info = map_object_at(&generated.floor, xf.to_world(pos), app.zoom);
                    if !info.is_empty() {
                        response.on_hover_text_at_pointer(info);
                    }
                }
            }
        });
}

fn zoom_at(app: &mut App, xf: &Xf, anchor: egui::Pos2, factor: f64) {
    let world = xf.to_world(anchor);
    let zoom = (app.zoom * factor).clamp(0.3, 20.0);
    if (zoom - app.zoom).abs() < 1e-9 {
        return;
    }
    app.zoom = zoom;
    // Shift the pan so the world point under the cursor stays under it at the new zoom.
    if let Some((min, max)) = app.map_bounds() {
        let moved = Xf::new(xf.rect(), min, max, app.zoom, app.pan).to_world(anchor);
        app.pan[0] += world[0] - moved[0];
        app.pan[1] += world[1] - moved[1];
    }
}

fn toolbar(app: &mut App, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("−").on_hover_text("Zoom out").clicked() {
            app.zoom = (app.zoom * 0.8).max(0.3);
        }
        ui.label(format!("{}%", (app.zoom * 100.0).round() as u32));
        if ui.button("+").on_hover_text("Zoom in").clicked() {
            app.zoom = (app.zoom * 1.25).min(20.0);
        }
        if ui
            .button("Fit")
            .on_hover_text("Double-click on the map")
            .clicked()
        {
            app.zoom = 1.0;
            app.pan = [0.0, 0.0];
        }
        ui.separator();
        ui.label(muted(ui, "Layers"));
        let o = &mut app.overlays;
        for (flag, name, hint) in [
            (&mut o.cells, "Grid", "Grid blocks of the floor"),
            (
                &mut o.labels,
                "Labels",
                "Number, shape and role of each room",
            ),
            (&mut o.ports, "Doors", "Every possible door position"),
            (&mut o.pairs, "Pairs", "Candidate room pairs for connection"),
            (&mut o.candidates, "Routes", "Candidate corridor routes"),
            (&mut o.legend, "Legend", "Shape and role colors"),
        ] {
            if ui
                .selectable_label(*flag, name)
                .on_hover_text(hint)
                .clicked()
            {
                *flag = !*flag;
            }
        }
    });
}
