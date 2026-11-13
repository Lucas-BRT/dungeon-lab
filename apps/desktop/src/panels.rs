//! The window's layout: top bar, bottom bar, the tabbed side panel and the map.

use crate::analysis::analysis_tab;
use crate::map::{self, Palette, Xf};
use crate::params_ui::{field_row, FieldKind, ParamField};
use crate::pick::map_object_at;
use crate::theme;
use crate::{
    format_number, get_path, set_path, size_description, size_label, translate_violation, App,
    Experiment, GeneratedMap, Tab,
};
use eframe::egui::{
    self, Align, Button, Color32, CursorIcon, Frame, Grid, Layout, Panel, RichText, ScrollArea,
    Sense, Stroke, TextEdit, Ui, Vec2,
};
use serde_json::{json, Value};
use tapiti_core::par;
use tapiti_core::params::Size;

pub(crate) fn muted(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .small()
        .color(ui.visuals().weak_text_color())
}

pub(crate) fn card(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    theme::card_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(15.0));
        ui.add_space(theme::GAP);
        body(ui);
    });
    ui.add_space(theme::GAP + 2.0);
}

fn primary(ui: &Ui, text: &str) -> Button<'static> {
    Button::new(
        RichText::new(text.to_string())
            .strong()
            .color(Color32::WHITE),
    )
    .fill(theme::of(ui).border_focused)
}

pub fn menu_bar(app: &mut App, ui: &mut Ui) {
    Panel::top("menu")
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("View", |ui| {
                    let mut theme = ui.options(|o| o.theme_preference);
                    for (value, name) in [
                        (egui::ThemePreference::Dark, "Dark theme"),
                        (egui::ThemePreference::Light, "Light theme"),
                        (egui::ThemePreference::System, "Follow the system"),
                    ] {
                        ui.radio_value(&mut theme, value, name);
                    }
                    ui.ctx().set_theme(theme);
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Shortcuts").clicked() {
                        app.show_shortcuts = true;
                        ui.close();
                    }
                });
            });
        });
}

pub fn shortcuts_window(app: &mut App, ctx: &egui::Context) {
    egui::Window::new("Shortcuts")
        .open(&mut app.show_shortcuts)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            for line in [
                "Alt+1–4 switches the size · Alt+R random seed",
                "Alt+Left / Right: previous / next seed",
                "Esc cancels a generation",
                "Drag moves the map · wheel zooms at the cursor",
                "Double-click fits the map to the window",
            ] {
                ui.label(line);
            }
        });
}

pub fn top_bar(app: &mut App, ui: &mut Ui) {
    Panel::top("top")
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Tapiti");
                ui.separator();
                layout_and_size(app, ui);
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
                if app.busy && app.can_cancel() {
                    if ui
                        .add(
                            Button::new(RichText::new("Cancel").strong().color(Color32::WHITE))
                                .fill(Palette::new(ui.visuals().dark_mode).danger),
                        )
                        .on_hover_text("Stop this generation (Esc). The floor on the screen stays.")
                        .clicked()
                    {
                        app.cancel_generation();
                    }
                } else if ui
                    .add_enabled(!app.busy, primary(ui, "Generate"))
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
    Panel::bottom("bottom")
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(muted(ui, &app.status));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(muted(ui, format!("{} threads", par::threads())));
                });
            });
        });
}

/// The layout and the size, side by side in the top bar: the choices that decide what the rest of the settings mean. Columns, rows,
/// leaf chambers and the growth settings live in their group of the side panel.
fn layout_and_size(app: &mut App, ui: &mut Ui) {
    let mut layout = None;
    let mut size = None;
    ui.label(RichText::new("Layout").strong());
    if let Some(field) = app
        .groups
        .iter()
        .flat_map(|g| &g.fields)
        .find(|f| f.path == "layout")
    {
        if let FieldKind::Choice(options) = &field.kind {
            let current = get_path(
                &serde_json::to_value(&app.params).unwrap_or(Value::Null),
                "layout",
            )
            .and_then(Value::as_str)
            .unwrap_or("grid")
            .to_string();
            for (value, name) in options {
                if ui
                    .selectable_label(*value == current, name.as_str())
                    .on_hover_text(field.help.as_str())
                    .clicked()
                    && *value != current
                {
                    layout = Some(Value::String(value.clone()));
                }
            }
        }
    }
    ui.separator();
    ui.label(RichText::new("Size").strong());
    for (n, s) in Size::ALL.into_iter().enumerate() {
        let hint = format!("{}\nAlt+{}", size_description(s), n + 1);
        if ui
            .selectable_label(app.size == s, size_label(s))
            .on_hover_text(hint)
            .clicked()
        {
            size = Some(s);
        }
    }
    if let Some(s) = size {
        app.set_size(s);
    } else if let Some(value) = layout {
        app.change_param("layout", value);
    }
}

/// Whether a group of parameters applies to the layout: the growth settings only to the grown layout.
fn group_applies(id: &str, layout: tapiti_core::params::Layout) -> bool {
    id != "growth" || layout == tapiti_core::params::Layout::Growth
}

pub fn side_panel(app: &mut App, ui: &mut Ui) {
    Panel::right("side")
        .frame(theme::panel_frame(ui))
        .default_size(400.0)
        .size_range(340.0..=600.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            // Four tabs that fit one row: the parameters are one list of collapsible groups, not a tab each.
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.horizontal(|ui| {
                    for (tab, title) in [
                        (Tab::Floor, "Floor"),
                        (Tab::Settings, "Parameters"),
                        (Tab::Analysis, "Analysis"),
                        (Tab::Advanced, "Advanced"),
                    ] {
                        if theme::tab(ui, title, app.tab == tab).clicked() {
                            app.tab = tab;
                        }
                    }
                });
            });
            ui.add_space(theme::GAP);
            // The clear button first, from the right, and the box takes what is left: a box sized as "the width left minus a guess" is
            // a few pixels too wide, and in a side panel (which grows to fit its content) that made the panel widen at every repaint.
            // (Inside a `horizontal`, so the right-to-left layout takes the height of one row, not what is left of the panel.)
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(!app.filter.is_empty(), Button::new("×"))
                        .on_hover_text("Clear the search")
                        .clicked()
                    {
                        app.filter.clear();
                    }
                    let search = ui.add(
                        TextEdit::singleline(&mut app.filter)
                            .hint_text("Search every parameter…")
                            .desired_width(ui.available_width()),
                    );
                    if search.changed() && !app.filter.trim().is_empty() {
                        app.tab = Tab::Settings;
                    }
                });
            });
            ui.separator();
            // Each tab keeps its own scroll position and starts at the top: with one shared area, leaving a long tab (the Analysis, scrolled
            // down) for a short one left the content to slide back up to fit.
            let mut area = ScrollArea::vertical()
                .id_salt(app.tab as u8)
                .auto_shrink(false);
            if app.generated.is_some() {
                if let Some(offset) = app.scroll_to.take() {
                    area = area.vertical_scroll_offset(offset);
                }
            }
            area.show(ui, |ui| match app.tab {
                Tab::Floor => floor_tab(app, ui),
                Tab::Settings => settings_tab(app, ui),
                Tab::Advanced => advanced_tab(app, ui),
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
                        ("Rooms", (m.rooms + m.leaf_chambers).to_string()),
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
}

// ---- Settings: one list of collapsible groups --------------------------------------------------

/// The parameters as one list of collapsible groups: Grid and size and Room shapes open, the others closed, a count of the changed
/// ones in each title. A search opens every group that has a match and hides the rest.
fn settings_tab(app: &mut App, ui: &mut Ui) {
    let groups = std::mem::take(&mut app.groups);
    let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
    let needle = app.filter.trim().to_lowercase();
    let layout = app.params.layout;
    let focus = app.focus_group.take();
    let mut edits: Vec<(String, Value)> = Vec::new();
    let mut reset = None;
    let mut shown = 0;
    for group in groups.iter().filter(|g| group_applies(&g.id, layout)) {
        // The layout is chosen in the top bar.
        let fields: Vec<&ParamField> = group
            .fields
            .iter()
            .filter(|f| f.path != "layout")
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
            .filter(|f| f.path != "layout" && app.changed.contains(&f.path))
            .count();
        let title = if changed > 0 {
            format!("{} · {changed}", group.title.as_str())
        } else {
            group.title.as_str().to_string()
        };
        let focused = focus.as_deref() == Some(group.id.as_str());
        let forced = (focused || !needle.is_empty()).then_some(true);
        let response = egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt(("group", &group.id))
            .default_open(matches!(group.id.as_str(), "scaffold" | "shapes"))
            .open(forced)
            .show(ui, |ui| {
                ui.label(muted(ui, group.intro.as_str()));
                ui.add_space(4.0);
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
                ui.add_space(6.0);
            });
        if focused {
            response.header_response.scroll_to_me(Some(Align::TOP));
        }
    }
    app.groups = groups;
    if shown == 0 {
        ui.label(muted(ui, "No parameter matches the search."));
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
}

/// The tools that are not one group of parameters: copying the Λ, going back to the size, the guided experiments and the JSON.
fn advanced_tab(app: &mut App, ui: &mut Ui) {
    ui.label(muted(
        ui,
        "Every change regenerates the floor after a short pause. Hover a name to see what it does.",
    ));
    ui.add_space(4.0);
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

    ui.add_space(6.0);
    ui.label(RichText::new("Guided experiments").strong());
    ui.label(muted(
        ui,
        "One click changes a few parameters to show what they do.",
    ));
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
    egui::CollapsingHeader::new(RichText::new("JSON editor").strong())
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

// ---- Map ----------------------------------------------------------------------------------------

pub fn central(app: &mut App, ui: &mut Ui) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::of(ui).panel).inner_margin(10.0))
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
