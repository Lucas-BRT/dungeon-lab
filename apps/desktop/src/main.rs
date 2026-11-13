//! Native desktop frontend (egui) for Tapiti.
//!
//! Unlike the browser build, this binary calls the Rust generator directly with the
//! `parallel` feature enabled. Generation runs on a background OS thread so the
//! window remains responsive while Rayon handles the generator's parallel phases.

// A release build on Windows is a GUI program: without this it opens a console window next to the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod charts;
mod map;
mod panels;
mod params_ui;
mod pick;
mod theme;

use eframe::egui::{self, Key, Modifiers};
use params_ui::ParamGroup;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tapiti_core::cancel::Cancel;
use tapiti_core::generate::{Floor, GenerationEvent, Role};
use tapiti_core::geom::Shape;
use tapiti_core::metrics::{metrics_of, Metrics};
use tapiti_core::par;
use tapiti_core::params::{params_of_size, Params, Size};
use tapiti_core::stage;

const APP_ID: &str = "dev.lucasbrt.Tapiti";
/// Pause after the last edit before the map regenerates.
const APPLY_DELAY: Duration = Duration::from_millis(180);
/// A generation shorter than this swaps the map in one step; a longer one shows its progress.
const PREVIEW_AFTER: Duration = Duration::from_millis(120);

#[derive(Clone)]
pub struct GeneratedMap {
    pub floor: Arc<Floor>,
    pub metrics: Metrics,
    pub violations: Vec<String>,
    pub draws: u32,
    pub elapsed_ms: f64,
    pub size: Size,
    pub seed: u32,
    pub phases: String,
}

#[derive(Default)]
pub struct BuildingMap {
    seed: u32,
    draw: u32,
    grid_x: Vec<f64>,
    grid_y: Vec<f64>,
    corridor_width: f64,
    rooms: Vec<Shape>,
    edges: Vec<Vec<[f64; 2]>>,
    connections: usize,
}

impl BuildingMap {
    fn has_layout(&self) -> bool {
        self.grid_x.len() >= 2 && self.grid_y.len() >= 2
    }

    fn bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        Some((
            [*self.grid_x.first()?, *self.grid_y.first()?],
            [*self.grid_x.last()?, *self.grid_y.last()?],
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Experiment {
    Shapes,
    Cycles,
    Junctions,
    CircularRooms,
}

#[derive(Debug, Clone, Copy)]
pub enum BenchmarkKind {
    Forty,
    Batch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The groups of parameters: Grid and size, Room shapes…
    Settings,
    Analysis,
}

#[derive(Debug, Clone, Copy)]
pub struct Overlays {
    pub cells: bool,
    pub labels: bool,
    pub ports: bool,
    pub pairs: bool,
    pub candidates: bool,
    pub legend: bool,
}

/// What a worker thread tells the UI; always tagged with the work id it belongs to.
#[allow(clippy::large_enum_variant)]
enum Event {
    Progress(Vec<GenerationEvent>),
    Generated(Result<GeneratedMap, String>),
    /// The generation was stopped by the user before it was done.
    Cancelled,
    Benchmark(String),
}

pub struct App {
    ctx: egui::Context,
    tx: Sender<(u64, Event)>,
    rx: Receiver<(u64, Event)>,
    pub tab: Tab,
    pub size: Size,
    pub seed: u32,
    pub seed_draft: String,
    pub params: Params,
    pub defaults: Value,
    /// Paths of the parameters that differ from the size default.
    pub changed: HashSet<String>,
    pub json_text: String,
    pub json_dirty: bool,
    pub json_error: Option<String>,
    pub show_json: bool,
    pub filter: String,
    pub overlays: Overlays,
    pub zoom: f64,
    pub pan: [f64; 2],
    pub busy: bool,
    pub queued: bool,
    /// Stops the generation that is running, when there is one.
    cancel: Option<Cancel>,
    work_id: u64,
    launched_at: Instant,
    show_building: bool,
    show_shortcuts: bool,
    apply_at: Option<Instant>,
    pub status: String,
    pub performance_status: Option<String>,
    pub generated: Option<GeneratedMap>,
    pub building: BuildingMap,
    pub groups: Vec<ParamGroup>,
    /// `TAPITI_SCREENSHOT=file.ppm`: writes the window once the first floor is drawn and quits (for the pictures of the docs and for checks).
    shot: Option<Shot>,
    /// `TAPITI_SCROLL`: where the side panel starts, for a screenshot of the part below the fold.
    pub scroll_to: Option<f32>,
    /// A group of parameters to open and scroll to once (the screenshots of the docs ask for it).
    pub focus_group: Option<String>,
}

struct Shot {
    path: String,
    frames: u32,
    requested: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        let params = params_of_size(Size::Small);
        let mut app = Self {
            ctx: cc.egui_ctx.clone(),
            tx,
            rx,
            tab: Tab::Settings,
            size: Size::Small,
            seed: 1,
            seed_draft: "1".into(),
            defaults: serde_json::to_value(&params).unwrap_or(Value::Null),
            params,
            changed: HashSet::new(),
            json_text: String::new(),
            json_dirty: false,
            json_error: None,
            show_json: false,
            filter: String::new(),
            overlays: Overlays {
                cells: true,
                labels: true,
                ports: false,
                pairs: false,
                candidates: false,
                legend: true,
            },
            zoom: 1.0,
            pan: [0.0, 0.0],
            busy: false,
            queued: false,
            cancel: None,
            work_id: 0,
            launched_at: Instant::now(),
            show_building: false,
            show_shortcuts: false,
            apply_at: None,
            status: "Preparing the generator…".into(),
            performance_status: None,
            generated: None,
            building: BuildingMap::default(),
            groups: params_ui::groups(),
            focus_group: None,
            scroll_to: std::env::var("TAPITI_SCROLL")
                .ok()
                .and_then(|v| v.parse().ok()),
            shot: std::env::var("TAPITI_SCREENSHOT").ok().map(|path| Shot {
                path,
                frames: 0,
                requested: false,
            }),
        };
        app.pick_from_env();
        app.sync_json();
        app.launch_generate();
        app
    }

    /// `TAPITI_LAYOUT`, `TAPITI_SIZE` (1 to 4) and `TAPITI_TAB` (analysis, parameters or the id of a group) choose what a screenshot shows.
    fn pick_from_env(&mut self) {
        if self.shot.is_none() {
            return;
        }
        if let Ok(size) = std::env::var("TAPITI_SIZE") {
            if let Ok(n) = size.parse::<usize>() {
                self.size = Size::of_index(n);
                self.params = params_of_size(self.size);
                self.defaults = serde_json::to_value(&self.params).unwrap_or(Value::Null);
            }
        }
        if std::env::var("TAPITI_LAYOUT").is_ok_and(|v| v == "growth") {
            self.params.layout = tapiti_core::params::Layout::Growth;
        }
        if std::env::var("TAPITI_THEME").is_ok_and(|v| v == "light") {
            self.ctx.set_theme(egui::ThemePreference::Light);
        }
        if let Some(zoom) = std::env::var("TAPITI_ZOOM")
            .ok()
            .and_then(|z| z.parse::<f32>().ok())
        {
            self.ctx.set_zoom_factor(zoom);
        }
        if let Ok(tab) = std::env::var("TAPITI_TAB") {
            match tab.as_str() {
                "floor" | "analysis" => self.tab = Tab::Analysis,
                "advanced" | "parameters" => self.tab = Tab::Settings,
                id => {
                    self.tab = Tab::Settings;
                    self.focus_group = Some(id.to_string());
                }
            }
        }
        self.refresh_changed();
    }

    /// Asks the window for a picture once the first floor is on it, writes it as a PPM and quits.
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        let ready = !self.busy && self.generated.is_some();
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        shot.frames += 1;
        if ready && shot.frames > 20 && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            ctx.request_screenshot(move |image| {
                let [w, h] = image.size;
                let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
                for pixel in &image.pixels {
                    out.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
                }
                let _ = std::fs::write(&path, out);
                std::process::exit(0);
            });
        }
    }

    // ---- generation -------------------------------------------------------------------------

    /// Starts a generation now, or after the running one when there is one.
    pub fn request_generate(&mut self) {
        if self.busy {
            self.queued = true;
        } else {
            self.launch_generate();
        }
    }

    fn launch_generate(&mut self) {
        self.busy = true;
        self.queued = false;
        self.work_id = self.work_id.wrapping_add(1);
        self.launched_at = Instant::now();
        self.show_building = false;
        self.building = BuildingMap::default();
        self.status = "Generating the map…".into();
        let (work_id, size, seed, params) =
            (self.work_id, self.size, self.seed, self.params.clone());
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let cancel = Cancel::new();
        self.cancel = Some(cancel.clone());
        std::thread::spawn(move || {
            // One message per frame keeps generation fast and avoids waking the UI for every room and corridor.
            let frame = Duration::from_millis(33);
            let mut events = Vec::new();
            let mut last_flush = Instant::now();
            let mut progress = |event| {
                events.push(event);
                if last_flush.elapsed() >= frame {
                    let _ = tx.send((work_id, Event::Progress(std::mem::take(&mut events))));
                    ctx.request_repaint();
                    last_flush = Instant::now();
                }
            };
            let result = generate_map(size, seed, params, &cancel, &mut progress);
            drop(progress);
            if !events.is_empty() {
                let _ = tx.send((work_id, Event::Progress(events)));
            }
            let _ = tx.send((
                work_id,
                match result {
                    Some(result) => Event::Generated(result),
                    None => Event::Cancelled,
                },
            ));
            ctx.request_repaint();
        });
    }

    /// Whether a generation is running that the Cancel button can stop (a benchmark is not).
    pub fn can_cancel(&self) -> bool {
        self.cancel.is_some()
    }

    /// Stops the generation that is running (a very large floor can take a long time) and forgets the one queued after it.
    pub fn cancel_generation(&mut self) {
        self.queued = false;
        self.apply_at = None;
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
            self.status = "Cancelling the generation…".into();
        }
    }

    pub fn start_benchmark(&mut self, kind: BenchmarkKind) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.work_id = self.work_id.wrapping_add(1);
        self.performance_status = Some("Measuring on the native Rust engine…".into());
        let (work_id, size) = (self.work_id, self.size);
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send((work_id, Event::Benchmark(run_benchmark(size, kind))));
            ctx.request_repaint();
        });
    }

    fn poll_events(&mut self) {
        while let Ok((id, event)) = self.rx.try_recv() {
            if id != self.work_id {
                continue;
            }
            match event {
                Event::Progress(events) => self.apply_progress(events),
                Event::Generated(result) => {
                    self.busy = false;
                    self.cancel = None;
                    self.show_building = false;
                    self.building = BuildingMap::default();
                    match result {
                        Ok(map) => {
                            self.status = format!(
                                "Floor generated in {ms:.1} ms · {draws} attempt(s).",
                                ms = map.elapsed_ms,
                                draws = map.draws
                            );
                            self.generated = Some(map);
                        }
                        Err(error) => {
                            self.status = format!("Generation failed: {error}", error = error)
                        }
                    }
                    if self.queued {
                        self.launch_generate();
                    }
                }
                Event::Cancelled => {
                    self.busy = false;
                    self.cancel = None;
                    self.show_building = false;
                    self.building = BuildingMap::default();
                    self.status =
                        "Generation cancelled; the floor on the screen is the previous one.".into();
                    if self.queued {
                        self.launch_generate();
                    }
                }
                Event::Benchmark(text) => {
                    self.busy = false;
                    self.performance_status = Some(text);
                    self.status = format!(
                        "Native engine · {threads} threads in the Rayon pool",
                        threads = par::threads()
                    );
                }
            }
        }
    }

    fn apply_progress(&mut self, events: Vec<GenerationEvent>) {
        // A quick generation replaces the map in one step; only a slow one shows its building.
        if self.launched_at.elapsed() >= PREVIEW_AFTER {
            self.show_building = true;
        }
        for event in events {
            match event {
                GenerationEvent::AttemptStarted { draw, seed } => {
                    self.building = BuildingMap {
                        draw,
                        seed,
                        ..BuildingMap::default()
                    };
                }
                GenerationEvent::Layout {
                    grid_x,
                    grid_y,
                    corridor_width,
                } => {
                    self.building.grid_x = grid_x;
                    self.building.grid_y = grid_y;
                    self.building.corridor_width = corridor_width;
                }
                GenerationEvent::RoomAdded(shape) => self.building.rooms.push(shape),
                GenerationEvent::EdgeUpdated { id, pts } => {
                    let b = &mut self.building;
                    if b.edges.len() <= id {
                        b.edges.resize_with(id + 1, Vec::new);
                    }
                    if b.edges[id].is_empty() {
                        b.connections += 1;
                    }
                    b.edges[id] = pts;
                }
            }
        }
        let b = &self.building;
        self.status = format!(
            "Attempt {draw} · seed {seed} · {rooms} rooms · {links} connections",
            draw = b.draw,
            seed = b.seed,
            rooms = b.rooms.len(),
            links = b.connections
        );
    }

    // ---- parameters -------------------------------------------------------------------------

    pub fn sync_json(&mut self) {
        self.json_text = serde_json::to_string_pretty(&self.params).unwrap_or_default();
        self.json_dirty = false;
        self.json_error = None;
    }

    fn refresh_changed(&mut self) {
        let current = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        self.changed = self
            .groups
            .iter()
            .flat_map(|group| &group.fields)
            .filter(|field| {
                get_path(&current, &field.path) != get_path(&self.defaults, &field.path)
            })
            .map(|field| field.path.clone())
            .collect();
    }

    /// Call after `params` changed from outside the JSON editor: refreshes derived state and schedules a regeneration.
    pub fn params_edited(&mut self) {
        self.refresh_changed();
        self.sync_json();
        self.apply_at = Some(Instant::now() + APPLY_DELAY);
    }

    pub fn set_size(&mut self, size: Size) {
        self.size = size;
        self.params = params_of_size(self.size);
        self.defaults = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        self.refresh_changed();
        self.sync_json();
        self.zoom = 1.0;
        self.pan = [0.0, 0.0];
        self.request_generate();
    }

    /// The size, the seed and the Λ as the JSON that File > Save and Copy JSON write.
    pub fn export_json(&self) -> String {
        let export = json!({ "size": size_label(self.size).to_lowercase(), "seed": self.seed, "params": self.params });
        serde_json::to_string_pretty(&export).unwrap_or_default()
    }

    /// File > Save Λ…: asks for a file and writes the export there. (The dialog blocks the window while it is open.)
    pub fn save_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save the Λ")
            .set_file_name("tapiti.json")
            .add_filter("JSON", &["json"])
            .save_file()
        else {
            return;
        };
        self.status = match std::fs::write(&path, self.export_json()) {
            Ok(()) => format!("Saved {}.", path.display()),
            Err(error) => format!("Could not save {}: {error}", path.display()),
        };
    }

    /// File > Open Λ…: reads a file written by Save or Copy JSON (or a bare Λ) and generates with it.
    pub fn open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open a Λ")
            .add_filter("JSON", &["json"])
            .pick_file()
        else {
            return;
        };
        let loaded = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
            .and_then(|value| {
                let params = value.get("params").unwrap_or(&value).clone();
                let params = serde_json::from_value::<Params>(params).map_err(|e| e.to_string())?;
                validate_params(&params)?;
                Ok((value, params))
            });
        match loaded {
            Ok((value, params)) => {
                if let Some(size) = value.get("size").and_then(Value::as_str).and_then(|name| {
                    Size::ALL
                        .into_iter()
                        .find(|s| size_label(*s).to_lowercase() == name)
                }) {
                    self.size = size;
                    self.defaults =
                        serde_json::to_value(params_of_size(size)).unwrap_or(Value::Null);
                }
                if let Some(seed) = value
                    .get("seed")
                    .and_then(Value::as_u64)
                    .and_then(|s| u32::try_from(s).ok())
                {
                    self.seed = seed;
                    self.seed_draft = seed.to_string();
                }
                self.params = params;
                self.refresh_changed();
                self.sync_json();
                self.status = format!("Opened {}.", path.display());
                self.request_generate();
            }
            Err(error) => self.status = format!("Could not open {}: {error}", path.display()),
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
        self.seed_draft = seed.to_string();
        self.request_generate();
    }

    pub fn random_seed(&mut self) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or_default();
        self.set_seed(seed);
    }

    pub fn commit_seed_draft(&mut self) {
        match self.seed_draft.trim().parse::<u32>() {
            Ok(seed) => self.set_seed(seed),
            Err(_) => {
                self.seed_draft = self.seed.to_string();
                self.status = "The seed must be an integer between 0 and 4,294,967,295.".into();
            }
        }
    }

    /// Takes the JSON editor's text as the new Λ, if it is valid.
    fn commit_json(&mut self) -> bool {
        let parsed = serde_json::from_str::<Params>(&self.json_text)
            .map_err(|error| format!("Invalid Λ; check the JSON: {error}", error = error))
            .and_then(|params| validate_params(&params).map(|()| params));
        match parsed {
            Ok(params) => {
                self.params = params;
                self.json_dirty = false;
                self.json_error = None;
                self.refresh_changed();
                true
            }
            Err(error) => {
                self.json_error = Some(error);
                false
            }
        }
    }

    pub fn change_param(&mut self, path: &str, value: Value) {
        let mut params = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        set_path(&mut params, path, value);
        match serde_json::from_value::<Params>(params) {
            Ok(params) => {
                self.params = params;
                self.params_edited();
            }
            Err(error) => self.status = format!("Invalid parameter: {error}", error = error),
        }
    }

    pub fn reset_group(&mut self, id: &str) {
        let Some(group) = self.groups.iter().find(|group| group.id == id) else {
            return;
        };
        let mut value = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        // The layout has its own strip: a tab's reset does not switch it back.
        for field in group.fields.iter().filter(|f| f.path != "layout") {
            if let Some(default) = get_path(&self.defaults, &field.path).cloned() {
                set_path(&mut value, &field.path, default);
            }
        }
        let title = group.title.as_str().to_string();
        if let Ok(params) = serde_json::from_value(value) {
            self.params = params;
            self.params_edited();
            self.status = format!("{title} parameters restored.", title = title);
        }
    }

    pub fn set_preset(&mut self, preset: Experiment) {
        let p = &mut self.params;
        let note = match preset {
            Experiment::Shapes => {
                p.weights.rect = 1.0;
                p.weights.disc = 1.0;
                p.weights.oct = 1.0;
                p.weights.l = 1.0;
                p.weights.star = 1.0;
                p.m = 8;
                "varied shapes and diagonal directions"
            }
            Experiment::Cycles => {
                p.delta = 0.8;
                "high cycle density"
            }
            Experiment::Junctions => {
                p.p_t = p.p_t.max(0.25);
                p.p_x = p.p_x.max(0.15);
                "T and X junctions favored"
            }
            Experiment::CircularRooms => {
                p.weights.rect = 0.0;
                p.weights.disc = 1.0;
                p.weights.oct = 0.0;
                p.weights.l = 0.0;
                p.weights.star = 0.0;
                "preference for circular rooms"
            }
        };
        self.params_edited();
        self.status = format!("Experiment applied: {note}.", note = note);
    }

    pub fn map_bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        match (&self.generated, self.show_building) {
            (Some(map), false) => Some((
                [*map.floor.grid_x.first()?, *map.floor.grid_y.first()?],
                [*map.floor.grid_x.last()?, *map.floor.grid_y.last()?],
            )),
            _ => self.building.bounds().or_else(|| {
                let d = &self.generated.as_ref()?.floor;
                Some((
                    [*d.grid_x.first()?, *d.grid_y.first()?],
                    [*d.grid_x.last()?, *d.grid_y.last()?],
                ))
            }),
        }
    }

    pub fn showing_building(&self) -> bool {
        (self.show_building || self.generated.is_none()) && self.building.has_layout()
    }

    // ---- frame ------------------------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let alt = |ctx: &egui::Context, key| ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key));
        for (index, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4]
            .into_iter()
            .enumerate()
        {
            if alt(ctx, key) {
                self.set_size(Size::ALL[index]);
            }
        }
        if self.busy && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.cancel_generation();
        }
        if alt(ctx, Key::R) {
            self.random_seed();
        }
        if alt(ctx, Key::ArrowLeft) {
            self.set_seed(self.seed.saturating_sub(1));
        }
        if alt(ctx, Key::ArrowRight) {
            self.set_seed(self.seed.saturating_add(1));
        }
    }

    fn apply_due(&mut self, ctx: &egui::Context) {
        let Some(at) = self.apply_at else { return };
        let now = Instant::now();
        if now < at {
            ctx.request_repaint_after(at - now);
            return;
        }
        self.apply_at = None;
        if self.json_dirty && !self.commit_json() {
            return;
        }
        self.request_generate();
    }

    /// The JSON editor changed: validate and regenerate after the usual pause.
    pub fn apply_at_edit(&mut self) {
        self.apply_at = Some(Instant::now() + APPLY_DELAY);
    }

    pub fn generate_now(&mut self) {
        self.apply_at = None;
        if self.json_dirty && !self.commit_json() {
            self.status = "The parameter JSON has errors; fix it to generate.".into();
            return;
        }
        if self.json_dirty {
            self.sync_json();
        }
        self.request_generate();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_events();
        self.shortcuts(&ctx);
        self.apply_due(&ctx);
        panels::menu_bar(self, ui);
        panels::top_bar(self, ui);
        panels::bottom_bar(self, ui);
        panels::side_panel(self, ui);
        panels::central(self, ui);
        panels::shortcuts_window(self, &ctx);
        panels::json_window(self, &ctx);
        self.take_screenshot(&ctx);
    }
}

fn setup_style(ctx: &egui::Context) {
    theme::apply(ctx);
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title("Tapiti")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
                    .expect("assets/icon.png is a valid PNG"),
            )
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tapiti",
        options,
        Box::new(|cc| {
            setup_style(&cc.egui_ctx);
            Ok(Box::new(App::new(cc)))
        }),
    )
}

// ---- helpers kept from the previous frontend ----------------------------------------------------

pub fn translate_violation(message: &str) -> String {
    if let Some((code, detail)) = message.split_once(' ') {
        let meaning = match code {
            "V1" => "room reach",
            "V2" => "distance and collisions",
            "V3" => "doors",
            "V4" => "corridor geometry",
            "V5" => "entrance and exit",
            "T1" | "T2" | "T3" | "T4" | "T5" => "topology requirement",
            _ => "model rule",
        };
        format!("{meaning}: {detail}")
    } else {
        message.to_string()
    }
}

fn translate_log(message: &str) -> String {
    if let Some(value) = message.strip_prefix("tree stuck: near ") {
        return format!(
            "The search widened the connection radius to {value}.",
            value = value
        );
    }
    if let Some(value) = message.strip_prefix("layout jitter limited to ") {
        return format!("Grid irregularity was limited to {value}.", value = value);
    }
    message.to_string()
}

fn class_label(class: &str) -> &str {
    match class {
        "rect" => "Rectangular",
        "disc" => "Circular",
        "oct" => "Octagonal",
        "L" => "L shape",
        "star" => "Truncated stars",
        _ => class,
    }
}

fn json_value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn get_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}

fn set_path(value: &mut Value, path: &str, next: Value) {
    let mut parts = path.split('.').peekable();
    let mut cursor = value;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if let Some(object) = cursor.as_object_mut() {
                object.insert(part.to_string(), next);
            }
            return;
        }
        let Some(object) = cursor.as_object_mut() else {
            return;
        };
        cursor = object
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
}

fn view_box(min: [f64; 2], max: [f64; 2], zoom: f64, pan: [f64; 2]) -> (f64, f64, f64, f64) {
    let width = (max[0] - min[0]).max(1.0);
    let height = (max[1] - min[1]).max(1.0);
    let margin = width.max(height) * 0.035;
    let width = (width + margin * 2.0) / zoom.max(0.05);
    let height = (height + margin * 2.0) / zoom.max(0.05);
    let center_x = (min[0] + max[0]) / 2.0 + pan[0];
    let center_y = (min[1] + max[1]) / 2.0 + pan[1];
    (
        center_x - width / 2.0,
        center_y - height / 2.0,
        width,
        height,
    )
}

fn run_benchmark(size: Size, kind: BenchmarkKind) -> String {
    let params = params_of_size(size);
    match kind {
        BenchmarkKind::Forty => {
            let started = Instant::now();
            let mut invalid = 0usize;
            for seed in 1..=40u32 {
                invalid += usize::from(!stage::stage(seed, &params, 8).violations.is_empty());
            }
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            format!(
                "Average: {ms:.2} ms per map · {invalid} invalid of 40 · {threads} threads available.",
                ms = elapsed / 40.0,
                invalid = invalid,
                threads = par::threads()
            )
        }
        BenchmarkKind::Batch => {
            let seeds: Vec<u32> = (1..=800).collect();
            let one_started = Instant::now();
            let one_invalid = seeds
                .iter()
                .map(|seed| usize::from(!stage::stage(*seed, &params, 8).violations.is_empty()))
                .sum::<usize>();
            let one_ms = one_started.elapsed().as_secs_f64() * 1000.0;
            let parallel_started = Instant::now();
            let parallel_invalid = par::pmap(&seeds, |seed| {
                usize::from(!stage::stage(*seed, &params, 8).violations.is_empty())
            })
            .into_iter()
            .sum::<usize>();
            let parallel_ms = parallel_started.elapsed().as_secs_f64() * 1000.0;
            format!(
                "800 maps: 1 thread {one:.0} ms · {threads} threads {many:.0} ms · invalid: {a}/{b}.",
                one = one_ms,
                threads = par::threads(),
                many = parallel_ms,
                a = one_invalid,
                b = parallel_invalid
            )
        }
    }
}

/// Multi-line description of a floor, built on demand so it follows the active language.
pub fn summary(map: &GeneratedMap) -> String {
    let m = &map.metrics;
    let validity = if map.violations.is_empty() {
        "valid"
    } else {
        "with violations"
    };
    let warnings = if map.violations.is_empty() {
        String::new()
    } else {
        format!("\nWarnings: {}", map.violations.join("; "))
    };
    format!(
        "{size} size · seed {seed} · {validity} · one floor generated in {ms:.1} ms · {draws} attempt(s)\n{rooms} rooms · {edges} corridors · β={beta} · δ={delta:.2} · {routes} routes to the exit\n{phases}{warnings}",
        size = size_label(map.size),
        seed = map.seed,
        validity = validity,
        ms = map.elapsed_ms,
        draws = map.draws,
        rooms = m.rooms,
        edges = m.edges,
        beta = m.beta,
        delta = m.delta,
        routes = m.routes_to_exit.map_or("no".to_string(), |n| n.to_string()),
        phases = map.phases,
        warnings = warnings
    )
}

/// `None` when the user cancelled before the floor was done.
fn generate_map(
    size: Size,
    seed: u32,
    params: Params,
    cancel: &Cancel,
    mut progress: impl FnMut(GenerationEvent),
) -> Option<Result<GeneratedMap, String>> {
    let started = Instant::now();
    let stage = stage::stage_cancellable(seed, &params, 8, cancel, &mut progress)?;
    if cancel.is_cancelled() {
        return None;
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let measures = metrics_of(&stage.d);
    let chosen_seed = stage.d.seed;
    let phases = stage
        .d
        .phases
        .iter()
        .map(|(name, ms)| format!("{name} {ms:.1} ms"))
        .collect::<Vec<_>>()
        .join(" · ");
    Some(Ok(GeneratedMap {
        floor: Arc::new(stage.d),
        metrics: measures,
        size,
        seed: chosen_seed,
        phases,
        violations: stage.violations,
        draws: stage.draws,
        elapsed_ms,
    }))
}

fn validate_params(p: &Params) -> Result<(), String> {
    fn float(name: &str, value: f64, min: f64, max: f64) -> Result<(), String> {
        if value.is_finite() && (min..=max).contains(&value) {
            Ok(())
        } else {
            Err(format!(
                "{name} must be between {min} and {max}.",
                name = name,
                min = min,
                max = max
            ))
        }
    }

    fn int(name: &str, value: i32, min: i32, max: i32) -> Result<(), String> {
        if (min..=max).contains(&value) {
            Ok(())
        } else {
            Err(format!(
                "{name} must be between {min} and {max}.",
                name = name,
                min = min,
                max = max
            ))
        }
    }

    fn count(name: &str, value: usize, max: usize) -> Result<(), String> {
        if value <= max {
            Ok(())
        } else {
            Err(format!(
                "{name} must be between 0 and {max}.",
                name = name,
                max = max
            ))
        }
    }

    fn by_class(name: &str, values: [(&str, f64); 5], min: f64, max: f64) -> Result<(), String> {
        for (class, value) in values {
            float(&format!("{name}.{class}"), value, min, max)?;
        }
        Ok(())
    }

    int("cols", p.cols, 1, 64)?;
    int("rows", p.rows, 1, 64)?;
    float("cellW", p.cell_w, 1.0, 5000.0)?;
    float("cellH", p.cell_h, 1.0, 5000.0)?;
    float("gap", p.gap, 0.0, 1000.0)?;
    int("big", p.big, 0, 1000)?;
    float("drop", p.drop, 0.0, 1.0)?;
    float("layoutJitter", p.layout_jitter, 0.0, 0.45)?;
    float("homophily", p.homophily, -1.0, 1.0)?;
    float("portStep", p.port_step, 0.1, 10000.0)?;
    float("width", p.width, 0.1, 10000.0)?;
    float("clear", p.clear, 0.0, 1000.0)?;
    float("exit", p.exit, 0.1, 10000.0)?;
    float("seg", p.seg, 0.1, 10000.0)?;
    float("minLen", p.min_len, 0.1, 10000.0)?;
    float("near", p.near, 0.1, 10000.0)?;
    float("delta", p.delta, 0.0, 1.0)?;
    if !matches!(p.m, 4 | 8) {
        return Err("m must be 4 or 8.".into());
    }
    if !matches!(p.disc_ports, 4 | 8 | 16) {
        return Err("discPorts must be 4, 8 or 16.".into());
    }
    if !matches!(p.max_bends, 0 | 1 | 2 | 4 | 5 | 6) {
        return Err("maxBends must be 0, 1, 2, 4, 5 or 6.".into());
    }
    int("growth.rooms", p.growth.rooms, 0, 2000)?;
    int("growth.branch", p.growth.branch, 1, 16)?;
    int("growth.depth", p.growth.depth, 1, 200)?;
    int("growth.islands", p.growth.islands, 1, 16)?;
    float("growth.cluster", p.growth.cluster, -10.0, 10.0)?;
    float("growth.reach", p.growth.reach, 0.0, 20.0)?;
    int("starCells", p.star_cells, 1, 1000)?;
    for (name, value) in [
        ("pI", p.p_i),
        ("pL", p.p_l),
        ("pZ", p.p_z),
        ("pW", p.p_w),
        ("pT", p.p_t),
        ("pX", p.p_x),
        ("costBend", p.cost_bend),
        ("costLen", p.cost_len),
        ("cost45", p.cost45),
        ("costFacing", p.cost_facing),
    ] {
        float(name, value, 0.0, 1000.0)?;
    }
    by_class(
        "weights",
        [
            ("rect", p.weights.rect),
            ("disc", p.weights.disc),
            ("oct", p.weights.oct),
            ("L", p.weights.l),
            ("star", p.weights.star),
        ],
        0.0,
        1000.0,
    )?;
    by_class(
        "maxOf",
        [
            ("rect", p.max_of.rect),
            ("disc", p.max_of.disc),
            ("oct", p.max_of.oct),
            ("L", p.max_of.l),
            ("star", p.max_of.star),
        ],
        0.0,
        10000.0,
    )?;
    for (class, value) in [
        ("rect", p.max_of.rect),
        ("disc", p.max_of.disc),
        ("oct", p.max_of.oct),
        ("L", p.max_of.l),
        ("star", p.max_of.star),
    ] {
        if value.fract() != 0.0 {
            return Err(format!("maxOf.{class} must be an integer.", class = class));
        }
    }
    by_class(
        "leafClasses",
        [
            ("rect", p.leaf_classes.rect),
            ("disc", p.leaf_classes.disc),
            ("oct", p.leaf_classes.oct),
            ("L", p.leaf_classes.l),
            ("star", p.leaf_classes.star),
        ],
        0.0,
        1000.0,
    )?;
    for (name, value) in &p.mix {
        float(&format!("mix.{name}"), *value, 0.0, 1.0)?;
    }
    if let Some(value) = p.junction {
        float("junction", value, 0.0, 10000.0)?;
    }
    int("exitDoors", p.exit_doors, 1, 1000)?;
    int("exitMinHops", p.exit_min_hops, 1, 1000)?;
    int("leaves", p.leaves, 0, 1000)?;
    count("topology.minBranches", p.topology.min_branches, 1000)?;
    count("topology.minBranchDepth", p.topology.min_branch_depth, 1000)?;
    count("topology.minExitRoutes", p.topology.min_exit_routes, 1000)?;
    count("topology.minCycleLength", p.topology.min_cycle_length, 1000)?;
    count("topology.minHubDegree", p.topology.min_hub_degree, 1000)?;
    Ok(())
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Entrance => "entrance",
        Role::Exit => "exit",
        Role::Plain => "plain",
    }
}

fn size_label(size: Size) -> &'static str {
    ["Small", "Medium", "Large", "Huge"][size as usize]
}

fn size_description(size: Size) -> String {
    let (cols, rows) = size.grid();
    format!(
        "{} grid of {cols} × {rows} blocks. Every other parameter keeps its default.",
        size_label(size)
    )
}
