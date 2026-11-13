//! The Analysis tab: what the generated floor is, in charts and plain words first, and every number for the curious below.

use crate::charts::{self, Bar};
use crate::map::{Palette, SHAPE_CLASSES};
use crate::panels::{card, muted};
use crate::{
    class_label, size_label, summary, translate_log, translate_violation, App, BenchmarkKind,
    GeneratedMap,
};
use eframe::egui::{self, Button, Color32, Frame, Grid, RichText, Ui};

/// A number with what it counts, in a rounded box.
fn tile(ui: &mut Ui, value: String, title: &str, caption: &str) {
    Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(value).strong().size(24.0));
        ui.label(RichText::new(title).strong());
        ui.label(muted(ui, caption));
    });
}

fn explain(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(muted(ui, text));
}

/// The friendly name of a phase of the generator.
fn phase_name(name: &str) -> &str {
    match name {
        "rooms" => "Placing the rooms",
        "candidates" => "Looking for corridors",
        "tree" => "Joining every room",
        "β_max" => "Measuring the room for loops",
        "cycles" => "Adding loops",
        "leaf chambers" => "Adding dead ends",
        other => other,
    }
}

pub fn analysis_tab(app: &mut App, ui: &mut Ui) {
    let Some(map) = app.generated.clone() else {
        ui.label(muted(ui, "Generate a floor to see what is in it."));
        performance(app, ui);
        return;
    };
    let pal = Palette::new(ui.visuals().dark_mode);
    let m = &map.metrics;

    // ---- the verdict, in words ----
    card(ui, "Does this floor work?", |ui| {
        if map.violations.is_empty() {
            ui.label(
                RichText::new("Yes: every check passes")
                    .strong()
                    .size(17.0)
                    .color(pal.success),
            );
            explain(
                ui,
                "Every room can be reached from the others, rooms and corridors keep their distance, and doors and corridors follow the shape rules.",
            );
        } else {
            ui.label(
                RichText::new(format!(
                    "Not quite: {n} rule(s) are not met",
                    n = map.violations.len()
                ))
                .strong()
                .size(17.0)
                .color(pal.danger),
            );
            explain(
                ui,
                &format!(
                    "The generator tried {n} time(s) and kept the best floor. It is drawn, but look at these before relying on it:",
                    n = map.draws
                ),
            );
            for violation in map.violations.iter().take(12) {
                ui.label(format!("• {}", translate_violation(violation)));
            }
        }
        ui.add_space(2.0);
        ui.label(muted(
            ui,
            format!(
                "{size} size · seed {seed}",
                size = size_label(app.size),
                seed = app.seed
            ),
        ));
    });

    // ---- at a glance ----
    card(ui, "At a glance", |ui| {
        ui.columns(2, |cols| {
            tile(
                &mut cols[0],
                (m.rooms + m.leaf_chambers).to_string(),
                "Rooms",
                &format!("{} main, {} dead-end chambers", m.rooms, m.leaf_chambers),
            );
            tile(
                &mut cols[1],
                m.edges.to_string(),
                "Corridor pieces",
                "What joins the rooms together",
            );
        });
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            tile(
                &mut cols[0],
                m.beta.to_string(),
                "Loops",
                "Extra ways to walk around instead of back",
            );
            tile(
                &mut cols[1],
                m.hops_to_exit.map_or("—".into(), |h| format!("{h:.0}")),
                "Rooms to the exit",
                "From the entrance, the shortest way",
            );
        });
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            tile(
                &mut cols[0],
                m.leaves.to_string(),
                "Rooms with one door",
                "Dead ends, the entrance and the exit",
            );
            tile(
                &mut cols[1],
                format!("{:.1} ms", map.elapsed_ms),
                "Time to build",
                &format!("{} attempt(s)", map.draws),
            );
        });
    });

    // ---- shapes ----
    card(ui, "Rooms by shape", |ui| {
        let bars: Vec<Bar> = SHAPE_CLASSES
            .iter()
            .filter_map(|class| {
                let n = *m.by_class.get(*class)?;
                Some(Bar::new(class_label(class), n as f64, pal.shape(class)))
            })
            .collect();
        let max = bars.iter().map(|b| b.value).fold(1.0, f64::max);
        charts::hbars(ui, &bars, max);
        explain(
            ui,
            "How varied the floor looks. The shape weights in the Room shapes tab change this mix.",
        );
    });

    // ---- doors per room ----
    card(ui, "Doors per room", |ui| {
        let mut counts = [0usize; 5];
        for room in map.floor.rooms.iter().filter(|r| r.alive) {
            counts[room.used.len().min(4)] += 1;
        }
        let labels = ["0", "1", "2", "3", "4+"];
        let bars: Vec<Bar> = counts
            .iter()
            .enumerate()
            .filter(|(i, n)| *i > 0 || **n > 0)
            .map(|(i, n)| {
                let color = match i {
                    1 => Color32::from_rgb(0xe0, 0x8a, 0x3c),
                    2 => pal.accent,
                    _ => pal.success,
                };
                Bar::new(labels[i], *n as f64, color)
            })
            .collect();
        let max = bars.iter().map(|b| b.value).fold(1.0, f64::max);
        charts::columns(ui, &bars, max, 130.0);
        explain(
            ui,
            "A room with one door is a dead end. Two doors make a hallway. Three or more make a crossroads, where a visitor chooses.",
        );
    });

    // ---- loops ----
    card(ui, "Ways around: loops", |ui| {
        let max = f64::from(m.beta.max(m.beta_target).max(m.beta_max).max(1));
        charts::hbars(
            ui,
            &[
                Bar::new("This floor", f64::from(m.beta), pal.accent),
                Bar::new("Aimed for", f64::from(m.beta_target), pal.success),
                Bar::new("Room for", f64::from(m.beta_max), pal.corridor),
            ],
            max,
        );
        explain(
            ui,
            "With no loops, every room has exactly one way in: a tree. Each loop gives a second way around. \"Aimed for\" follows the density setting; \"Room for\" is what this floor could hold at most.",
        );
    });

    // ---- the walk from the entrance to the exit ----
    card(ui, "From the entrance to the exit", |ui| {
        match m.hops_to_exit {
            None => {
                explain(
                ui,
                "This floor has no entrance and exit (see the Entrance and exit tab), so there is no walk to show.",
            );
            }
            Some(hops) => {
                charts::walk(
                    ui,
                    hops.round() as usize,
                    pal.success,
                    pal.accent,
                    pal.corridor,
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{hops:.0} rooms")).strong());
                    ui.label(muted(ui, "on the shortest way"));
                    if let Some(routes) = m.routes_to_exit {
                        ui.separator();
                        ui.label(RichText::new(format!("{routes} route(s)")).strong());
                        ui.label(muted(ui, "that share no corridor"));
                    }
                    if let Some(length) = m.geometric_path_to_exit {
                        ui.separator();
                        ui.label(RichText::new(format!("{length:.0} px")).strong());
                        ui.label(muted(ui, "of walking"));
                    }
                });
                explain(
                ui,
                "More than one independent route means a corridor that is blocked does not cut the exit off. The whole floor is at most this many rooms across:",
            );
                ui.label(RichText::new(format!("{:.0} rooms", m.diameter)).strong());
            }
        }
    });

    // ---- time ----
    if !map.floor.phases.is_empty() {
        card(ui, "Where the time went", |ui| {
            let palette = [
                pal.accent,
                pal.success,
                Color32::from_rgb(0xce, 0x84, 0x3c),
                Color32::from_rgb(0x9b, 0x78, 0xd1),
                Color32::from_rgb(0x29, 0xa8, 0x9a),
                Color32::from_rgb(0xd0, 0xae, 0x38),
            ];
            let parts: Vec<Bar> = map
                .floor
                .phases
                .iter()
                .enumerate()
                .map(|(i, (name, ms))| Bar::new(phase_name(name), *ms, palette[i % palette.len()]))
                .collect();
            charts::stacked(ui, &parts);
            explain(
                ui,
                "The last attempt, step by step. A floor that fails a check is built again from a nearby seed.",
            );
        });
    }

    // ---- the details ----
    egui::CollapsingHeader::new(RichText::new("All the numbers").strong())
        .id_salt("all-numbers")
        .show(ui, |ui| {
            for (title, rows) in metric_sections(&map) {
                ui.label(RichText::new(title).strong());
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
                ui.add_space(6.0);
            }
            ui.label(muted(ui, summary(&map)));
        });
    if !map.floor.log.is_empty() {
        egui::CollapsingHeader::new(RichText::new("What the generator did").strong())
            .id_salt("generator-log")
            .show(ui, |ui| {
                for line in &map.floor.log {
                    ui.label(muted(ui, translate_log(line)));
                }
            });
    }
    performance(app, ui);
}

fn performance(app: &mut App, ui: &mut Ui) {
    egui::CollapsingHeader::new(RichText::new("Speed of the generator").strong())
        .id_salt("performance")
        .show(ui, |ui| {
            ui.label(muted(
                ui,
                "Builds many floors of the selected size with its default settings and tells how fast it goes.",
            ));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!app.busy, Button::new("Measure 40 floors"))
                    .clicked()
                {
                    app.start_benchmark(BenchmarkKind::Forty);
                }
                if ui
                    .add_enabled(!app.busy, Button::new("Compare a batch of 800"))
                    .clicked()
                {
                    app.start_benchmark(BenchmarkKind::Batch);
                }
            });
            if let Some(text) = &app.performance_status {
                ui.label(text);
            }
        });
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
