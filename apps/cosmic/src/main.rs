//! Native COSMIC frontend for Dungeon Lab.
//!
//! Unlike the browser build, this binary calls the Rust generator directly with the
//! `parallel` feature enabled. Generation runs on a background OS thread so the
//! window remains responsive while Rayon handles the generator's parallel phases.

mod params_ui;

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::channel::oneshot;
use cosmic::iced::mouse::ScrollDelta;
use cosmic::iced::{Alignment, Color, Length, Point, Rectangle, Size, Vector};
use cosmic::prelude::*;
use cosmic::widget::{self, nav_bar};
use dungeon_lab_core::generate::{Dungeon, GenerationEvent, Node, Role};
use dungeon_lab_core::geom::{dir_of, dist, Shape};
use dungeon_lab_core::metrics::{metrics_of, Metrics};
use dungeon_lab_core::par;
use dungeon_lab_core::params::{params_of_level, Params};
use dungeon_lab_core::stage;
use params_ui::{FieldKind, ParamField, ParamGroup};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const APP_ID: &str = "dev.lucasbrt.DungeonLab";

#[derive(Clone)]
struct GeneratedMap {
    dungeon: Arc<Dungeon>,
    metrics: Metrics,
    violations: Vec<String>,
    draws: u32,
    elapsed_ms: f64,
    summary: String,
}

#[derive(Default)]
struct BuildingMap {
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

impl fmt::Debug for GeneratedMap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedMap")
            .field("seed", &self.dungeon.seed)
            .field("rooms", &self.metrics.rooms)
            .field("violations", &self.violations.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
enum Experiment {
    Shapes,
    Cycles,
    Junctions,
    CircularRooms,
}

#[derive(Debug, Clone, Copy)]
enum BenchmarkKind {
    Forty,
    Batch,
}

/// The native app follows COSMIC's navigation-shell convention: the map stays
/// focused while configuration and analysis live in their own adaptive pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StudioPage {
    Map,
    Parameters,
    Analysis,
}

#[derive(Debug, Clone, Copy, Default)]
struct Overlays {
    cells: bool,
    labels: bool,
    ports: bool,
    pairs: bool,
    candidates: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Generate,
    SelectLevel(usize),
    SetSeed(String),
    SeedDraft(String),
    ChangeSeed(i32),
    RandomSeed,
    ToggleParams,
    ParamsEdited(widget::text_editor::Action),
    ParamDraft(String, String),
    ParamValue(String, Value),
    CycleChoice(String),
    ToggleGroup(String),
    ResetGroup(String),
    ResetAll,
    ApplyPending(u64),
    ToggleOverlay(String, bool),
    Preset(Experiment),
    CopyParams,
    Benchmark(BenchmarkKind),
    BenchmarkFinished(u64, String),
    KeyboardLevel(usize),
    KeyboardSeed(i32),
    KeyboardRandomSeed,
    Zoom(f64),
    FitMap,
    MapPressed,
    MapReleased,
    MapPointerMoved { point: Point, size: Size },
    MapScrolled(ScrollDelta),
    Generated(u64, Result<GeneratedMap, String>),
    GenerationProgress(u64, Vec<GenerationEvent>),
}

struct App {
    core: cosmic::Core,
    nav_model: nav_bar::Model,
    level: usize,
    seed: u32,
    seed_draft: String,
    show_params: bool,
    params_editor: widget::text_editor::Content<cosmic::Renderer>,
    params: Params,
    defaults: Params,
    field_drafts: HashMap<String, String>,
    open_groups: HashSet<String>,
    changed_count: usize,
    overlays: Overlays,
    zoom: f64,
    pan: [f64; 2],
    map_size: Size,
    map_dragging: bool,
    last_pointer: Option<Point>,
    hover_info: String,
    busy: bool,
    work_id: u64,
    pending_revision: u64,
    status: String,
    performance_status: String,
    generated: Option<GeneratedMap>,
    building: Arc<RwLock<BuildingMap>>,
    /// Tessellated map, kept between frames: only a change to the map, view or overlays clears it.
    map_cache: cosmic::iced::widget::canvas::Cache,
    map_palette: RefCell<Option<MapPalette>>,
    /// The parameter groups, built once: the view reads them on every message.
    groups: Vec<ParamGroup>,
}

impl App {
    fn launch_generate(&mut self) -> Task<cosmic::Action<Message>> {
        if self.busy {
            return Task::none();
        }
        self.busy = true;
        self.work_id = self.work_id.wrapping_add(1);
        self.generated = None;
        self.building = Arc::new(RwLock::new(BuildingMap::default()));
        let work_id = self.work_id;
        self.status = "Preparando a grade do mapa…".into();
        let level = self.level;
        let seed = self.seed;
        let params = self.params.clone();
        let (sender, receiver) = mpsc::unbounded();
        std::thread::spawn(move || {
            // One progress message per frame keeps generation fast and avoids flooding the UI
            // with a full view/cache rebuild for every room and corridor.
            let frame = Duration::from_millis(33);
            let mut events = Vec::new();
            let mut last_flush = Instant::now();
            let mut progress = |event| {
                events.push(event);
                if last_flush.elapsed() >= frame {
                    let _ = sender.unbounded_send(Message::GenerationProgress(
                        work_id,
                        std::mem::take(&mut events),
                    ));
                    last_flush = Instant::now();
                }
            };
            let result = generate_map(level, seed, params, &mut progress);
            drop(progress);
            if !events.is_empty() {
                let _ = sender.unbounded_send(Message::GenerationProgress(work_id, events));
            }
            let _ = sender.unbounded_send(Message::Generated(work_id, result));
        });
        Task::run(receiver, cosmic::Action::App)
    }

    fn schedule_apply(&mut self) -> Task<cosmic::Action<Message>> {
        self.pending_revision = self.pending_revision.wrapping_add(1);
        let revision = self.pending_revision;
        Task::perform(
            async move {
                tokio::time::sleep(Duration::from_millis(180)).await;
                revision
            },
            |revision| cosmic::Action::App(Message::ApplyPending(revision)),
        )
    }

    fn sync_json_from_params(&mut self) {
        let text = serde_json::to_string_pretty(&self.params).unwrap_or_default();
        self.params_editor = widget::text_editor::Content::with_text(&text);
    }

    fn map_bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        if let Some(map) = &self.generated {
            Some((
                [*map.dungeon.grid_x.first()?, *map.dungeon.grid_y.first()?],
                [*map.dungeon.grid_x.last()?, *map.dungeon.grid_y.last()?],
            ))
        } else {
            self.building.read().ok()?.bounds()
        }
    }

    fn refresh_changed_count(&mut self) {
        let current = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        let defaults = serde_json::to_value(&self.defaults).unwrap_or(Value::Null);
        self.changed_count = params_ui::groups()
            .into_iter()
            .flat_map(|group| group.fields)
            .filter(|field| get_path(&current, &field.path) != get_path(&defaults, &field.path))
            .count();
    }

    fn current_level(&mut self, level: usize) -> Task<cosmic::Action<Message>> {
        self.level = level.clamp(1, 5);
        self.params = params_of_level(self.level);
        self.defaults = self.params.clone();
        self.field_drafts.clear();
        self.sync_json_from_params();
        self.refresh_changed_count();
        self.status = format!("Nível {} selecionado. Gerando o mapa…", self.level);
        self.launch_generate()
    }

    fn commit_drafts(&mut self) -> Result<(), String> {
        let mut value: Value = serde_json::from_str(&self.params_editor.text())
            .map_err(|error| format!("Λ inválida; revise o JSON: {error}"))?;
        for (path, text) in &self.field_drafts {
            let Some(field) = params_ui::field(path) else {
                continue;
            };
            let FieldKind::Number { min, max, integer } = field.kind else {
                continue;
            };
            let number = text
                .parse::<f64>()
                .map_err(|_| format!("{} precisa ser um número.", field.label))?;
            if !number.is_finite()
                || number < min
                || number > max
                || (integer && number.fract() != 0.0)
            {
                return Err(if integer {
                    format!("{} deve ser inteiro entre {min} e {max}.", field.label)
                } else {
                    format!("{} deve ficar entre {min} e {max}.", field.label)
                });
            }
            let number = if integer {
                json!(number as i64)
            } else {
                json!(number)
            };
            set_path(&mut value, &path, number);
        }
        let params: Params = serde_json::from_value(value)
            .map_err(|error| format!("Λ inválida; revise os campos: {error}"))?;
        validate_params(&params)?;
        self.params = params;
        self.field_drafts.clear();
        self.sync_json_from_params();
        self.refresh_changed_count();
        Ok(())
    }

    fn change_param(&mut self, path: String, value: Value) {
        let mut params = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        set_path(&mut params, &path, value);
        match serde_json::from_value::<Params>(params) {
            Ok(params) => {
                self.params = params;
                self.field_drafts.remove(&path);
                self.sync_json_from_params();
                self.refresh_changed_count();
                self.status = "Parâmetro alterado; o mapa será atualizado.".into();
            }
            Err(error) => self.status = format!("Parâmetro inválido: {error}"),
        }
    }

    fn reset_group(&mut self, id: &str) {
        let Some(group) = params_ui::groups().into_iter().find(|group| group.id == id) else {
            return;
        };
        let mut value = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        let defaults = serde_json::to_value(&self.defaults).unwrap_or(Value::Null);
        for field in group.fields {
            if let Some(default) = get_path(&defaults, &field.path).cloned() {
                set_path(&mut value, &field.path, default);
            }
            self.field_drafts.remove(&field.path);
        }
        if let Ok(params) = serde_json::from_value(value) {
            self.params = params;
            self.sync_json_from_params();
            self.refresh_changed_count();
            self.status = format!("Parâmetros de {} restaurados.", group.title);
        }
    }

    fn set_preset(&mut self, preset: Experiment) {
        match preset {
            Experiment::Shapes => {
                self.params.weights.rect = 1.0;
                self.params.weights.disc = 1.0;
                self.params.weights.oct = 1.0;
                self.params.weights.l = 1.0;
                self.params.weights.star = 1.0;
                self.params.m = 8;
                self.status = "Experimento aplicado: formas variadas e direções diagonais.".into();
            }
            Experiment::Cycles => {
                self.params.delta = 0.8;
                self.status = "Experimento aplicado: densidade de ciclos elevada.".into();
            }
            Experiment::Junctions => {
                self.params.p_t = self.params.p_t.max(0.25);
                self.params.p_x = self.params.p_x.max(0.15);
                self.status = "Experimento aplicado: junções T e X favorecidas.".into();
            }
            Experiment::CircularRooms => {
                self.params.weights.rect = 0.0;
                self.params.weights.disc = 1.0;
                self.params.weights.oct = 0.0;
                self.params.weights.l = 0.0;
                self.params.weights.star = 0.0;
                self.status = "Experimento aplicado: preferência por salas circulares.".into();
            }
        }
        self.field_drafts.clear();
        self.sync_json_from_params();
        self.refresh_changed_count();
    }

    fn start_benchmark(&mut self, kind: BenchmarkKind) -> Task<cosmic::Action<Message>> {
        if self.busy {
            return Task::none();
        }
        self.busy = true;
        self.work_id = self.work_id.wrapping_add(1);
        let id = self.work_id;
        let level = self.level;
        self.performance_status = "Medindo no motor Rust nativo…".into();
        let (sender, receiver) = oneshot::channel();
        std::thread::spawn(move || {
            let _ = sender.send(run_benchmark(level, kind));
        });
        Task::perform(
            async move {
                receiver
                    .await
                    .unwrap_or_else(|_| "Benchmark interrompido.".into())
            },
            move |result| cosmic::Action::App(Message::BenchmarkFinished(id, result)),
        )
    }

    fn zoom_map(
        &mut self,
        factor: f64,
        pointer: Option<Point>,
        size: Size,
    ) -> Task<cosmic::Action<Message>> {
        let Some((min, max)) = self.map_bounds() else {
            return Task::none();
        };
        let anchor = pointer.unwrap_or(Point::new(size.width / 2.0, size.height / 2.0));
        let world = pointer_world(min, max, self.zoom, self.pan, anchor, size);
        self.zoom = (self.zoom * factor).clamp(0.3, 20.0);
        if let Some(world) = world {
            let (_, _, width, height) = view_box(min, max, self.zoom, self.pan);
            let scale = (size.width / width as f32)
                .min(size.height / height as f32)
                .max(0.001);
            let center_x = world[0] - (anchor.x - size.width / 2.0) as f64 / scale as f64;
            let center_y = world[1] - (anchor.y - size.height / 2.0) as f64 / scale as f64;
            let base_x = (min[0] + max[0]) / 2.0;
            let base_y = (min[1] + max[1]) / 2.0;
            self.pan = [center_x - base_x, center_y - base_y];
        }
        Task::none()
    }

    fn active_page(&self) -> StudioPage {
        self.nav_model
            .active_data::<StudioPage>()
            .copied()
            .unwrap_or(StudioPage::Map)
    }

    /// The map takes the window; a narrow inspector holds the reading of the floor and the few controls worth reaching while looking at it.
    /// The Parameters page stays the full editor, and is only built when open.
    fn map_page(&self) -> Element<'_, Message> {
        widget::row::with_capacity(2)
            .push(self.map_preview())
            .push(
                widget::container(self.inspector())
                    .width(Length::Fixed(300.0))
                    .height(Length::Fill),
            )
            .spacing(12)
            .padding([8, 12])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn map_preview(&self) -> Element<'_, Message> {
        let toolbar = widget::row::with_capacity(6)
            .push(
                widget::button::icon(widget::icon::from_name("zoom-out-symbolic"))
                    .on_press(Message::Zoom(0.8)),
            )
            .push(
                widget::container(widget::text::body(format!(
                    "{}%",
                    (self.zoom * 100.0).round() as u32
                )))
                .width(Length::Fixed(52.0))
                .align_x(Alignment::Center),
            )
            .push(
                widget::button::icon(widget::icon::from_name("zoom-in-symbolic"))
                    .on_press(Message::Zoom(1.25)),
            )
            .push(widget::button::standard("Ajustar").on_press(Message::FitMap))
            .push(widget::Space::new().width(Length::Fill))
            .push(widget::text::caption("Arraste · roda = zoom"))
            .spacing(6)
            .align_y(Alignment::Center);

        let palette = MapPalette::current();
        let previous_palette = self.map_palette.replace(Some(palette.clone()));
        if previous_palette.as_ref() != Some(&palette) {
            self.map_cache.clear();
        }
        let has_map = self.generated.is_some()
            || self
                .building
                .read()
                .is_ok_and(|building| building.has_layout());
        let map_canvas: Element<'_, Message> = if has_map {
            let dungeon = self.generated.as_ref().map(|map| Arc::clone(&map.dungeon));
            let building = Arc::clone(&self.building);
            let (overlays, zoom, pan, dragging) =
                (self.overlays, self.zoom, self.pan, self.map_dragging);
            let cache = &self.map_cache;
            widget::responsive(move |size| {
                let canvas = widget::canvas(MapCanvas {
                    dungeon: dungeon.clone(),
                    building: Arc::clone(&building),
                    overlays,
                    zoom,
                    pan,
                    palette: palette.clone(),
                    cache,
                    dragging,
                })
                .width(Length::Fill)
                .height(Length::Fill);
                widget::mouse_area(canvas)
                    .on_press(Message::MapPressed)
                    .on_release(Message::MapReleased)
                    .on_double_click(Message::FitMap)
                    .on_move(move |point| Message::MapPointerMoved { point, size })
                    .on_scroll(Message::MapScrolled)
                    .into()
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            widget::container(widget::text::body("O mapa gerado será exibido aqui."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::Center)
                .align_x(Alignment::Center)
                .into()
        };

        let hover = if self.hover_info.is_empty() {
            "Passe sobre uma sala, junção ou corredor para ver detalhes.".to_string()
        } else {
            self.hover_info.clone()
        };
        widget::column::with_capacity(3)
            .push(toolbar)
            .push(
                widget::container(map_canvas)
                    .class(cosmic::theme::Container::Card)
                    .padding(2)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .push(
                widget::container(widget::text::caption(hover))
                    .height(Length::Fixed(72.0))
                    .align_y(Alignment::Center),
            )
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Side panel of the map page: reading of the floor, layout mode and map layers.
    fn inspector(&self) -> Element<'_, Message> {
        fn card<'a>(title: &str, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
            widget::container(
                widget::column::with_capacity(2)
                    .push(widget::text::title4(title.to_string()))
                    .push(body)
                    .spacing(8),
            )
            .class(cosmic::theme::Container::Card)
            .padding(12)
            .width(Length::Fill)
            .into()
        }

        // Reading of the floor.
        let reading: Element<'_, Message> = match &self.generated {
            None => widget::text::body(if self.busy {
                "Montando o andar…"
            } else {
                "Nenhum andar gerado."
            })
            .into(),
            Some(map) => {
                let m = &map.metrics;
                let mut col =
                    widget::column::with_capacity(8)
                        .spacing(4)
                        .push(widget::text::title4(if map.violations.is_empty() {
                            "✓ Válido".to_string()
                        } else {
                            format!("! {} problema(s)", map.violations.len())
                        }));
                for (label, value) in [
                    ("Salas", m.rooms.to_string()),
                    ("Corredores", m.edges.to_string()),
                    ("Ciclos β", format!("{} (alvo {})", m.beta, m.beta_target)),
                    ("Rotas até a escada", m.routes_to_stairs.to_string()),
                    ("Tentativas", map.draws.to_string()),
                    ("Geração", format!("{:.1} ms", map.elapsed_ms)),
                ] {
                    col = col.push(
                        widget::row::with_capacity(2)
                            .push(widget::text::caption(label).width(Length::Fill))
                            .push(widget::text::body(value)),
                    );
                }
                for violation in map.violations.iter().take(4) {
                    col = col.push(widget::text::caption(translate_violation(violation)));
                }
                col.into()
            }
        };

        // Layout mode: grid or grown, with the growth knobs when it is on.
        let current_json = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        let mut layout =
            widget::column::with_capacity(8).spacing(6).push(
                widget::toggler(self.params.growth.enabled)
                    .label(Some("Crescimento (protótipo)".to_string()))
                    .on_toggle_maybe((!self.busy).then_some(|value| {
                        Message::ParamValue("growth.enabled".into(), json!(value))
                    })),
            );
        if self.params.growth.enabled {
            let paths = [
                "growth.rooms",
                "growth.islands",
                "growth.branch",
                "growth.depth",
                "growth.cluster",
                "growth.reach",
            ];
            for field in self
                .groups
                .iter()
                .flat_map(|g| g.fields.iter())
                .filter(|f| paths.contains(&f.path.as_str()))
            {
                let value = self
                    .field_drafts
                    .get(&field.path)
                    .cloned()
                    .or_else(|| get_path(&current_json, &field.path).map(json_value_text))
                    .unwrap_or_default();
                layout = layout.push(param_field_view(field.clone(), value, self.busy));
            }
        } else {
            layout = layout.push(widget::text::caption("Uma sala por bloco da grade."));
        }
        layout = layout.push(widget::text::caption(level_description(self.level)));

        // Map layers.
        let mut layers = widget::column::with_capacity(5).spacing(6);
        for (key, label, enabled) in [
            ("cells", "Grade", self.overlays.cells),
            ("labels", "Rótulos", self.overlays.labels),
            ("ports", "Portas", self.overlays.ports),
            ("pairs", "Pares", self.overlays.pairs),
            ("candidates", "Rotas candidatas", self.overlays.candidates),
        ] {
            layers = layers.push(
                widget::toggler(enabled)
                    .label(Some(label.to_string()))
                    .on_toggle(move |value| Message::ToggleOverlay(key.into(), value)),
            );
        }

        widget::scrollable(
            widget::column::with_capacity(3)
                .push(card("Leitura", reading))
                .push(card("Layout", layout))
                .push(card("Camadas", layers))
                .spacing(10),
        )
        .height(Length::Fill)
        .into()
    }

    fn parameters_page(&self) -> Element<'_, Message> {
        let current_json = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        let mut content = widget::column::with_capacity(20)
            .push(widget::text::title2("Parâmetros do gerador"))
            .push(widget::text::caption(
                "A Λ é aplicada automaticamente depois de uma breve pausa. Cada campo explica seu efeito e aceita apenas valores válidos.",
            ))
            .push(
                widget::row::with_capacity(2)
                    .push(widget::button::text("Copiar parâmetros (JSON)").on_press(Message::CopyParams))
                    .push(
                        widget::button::text("Restaurar padrão do nível")
                            .on_press_maybe((!self.busy).then_some(Message::ResetAll)),
                    )
                    .spacing(8),
            )
            .push(widget::text::caption(if self.changed_count == 0 {
                "Configuração padrão do nível".into()
            } else {
                format!("{} parâmetro(s) alterado(s)", self.changed_count)
            }));

        content =
            content.push(
                widget::settings::section()
                    .title("Experimentos guiados")
                    .add(
                        widget::button::text("Misturar formas e diagonais").on_press_maybe(
                            (!self.busy).then_some(Message::Preset(Experiment::Shapes)),
                        ),
                    )
                    .add(widget::button::text("Criar mais rotas").on_press_maybe(
                        (!self.busy).then_some(Message::Preset(Experiment::Cycles)),
                    ))
                    .add(
                        widget::button::text("Favorecer junções T/X").on_press_maybe(
                            (!self.busy).then_some(Message::Preset(Experiment::Junctions)),
                        ),
                    )
                    .add(
                        widget::button::text("Usar salas circulares").on_press_maybe(
                            (!self.busy).then_some(Message::Preset(Experiment::CircularRooms)),
                        ),
                    ),
            );

        for group in self.groups.iter().cloned() {
            let open = self.open_groups.contains(&group.id);
            let marker = if open { "▾" } else { "▸" };
            let mut section = widget::column::with_capacity(group.fields.len() + 3)
                .push(
                    widget::button::text(format!(
                        "{marker} {} · {} campos",
                        group.title,
                        group.fields.len()
                    ))
                    .on_press(Message::ToggleGroup(group.id.clone())),
                )
                .push(widget::text::caption(group.intro.clone()));
            if open {
                section =
                    section.push(widget::button::text("Restaurar este grupo").on_press_maybe(
                        (!self.busy).then_some(Message::ResetGroup(group.id.clone())),
                    ));
                for field in group.fields {
                    let value = self
                        .field_drafts
                        .get(&field.path)
                        .cloned()
                        .or_else(|| get_path(&current_json, &field.path).map(json_value_text))
                        .unwrap_or_default();
                    section = section.push(param_field_view(field, value, self.busy));
                }
            }
            content = content.push(widget::settings::section().add(section.spacing(6)));
        }

        content = content
            .push(widget::text::caption(format!(
                "Condutância das junções: {} (informativa; ainda não altera a geração).",
                self.params
                    .junction
                    .map(|value| format_number(value, 2))
                    .unwrap_or_else(|| "∞, braços unidos".into())
            )))
            .push(
                widget::button::text(if self.show_params {
                    "Ocultar editor JSON avançado"
                } else {
                    "Abrir editor JSON avançado"
                })
                .on_press(Message::ToggleParams),
            );
        if self.show_params && self.busy {
            content = content.push(widget::text::caption(
                "O editor fica bloqueado enquanto há uma geração ou benchmark em andamento.",
            ));
        } else if self.show_params {
            content = content.push(
                widget::text_editor::text_editor(&self.params_editor)
                    .on_action(Message::ParamsEdited)
                    .height(Length::Fixed(300.0)),
            );
        }

        widget::scrollable(
            widget::container(content.spacing(16).padding(20))
                .max_width(940.0)
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .into()
    }

    fn analysis_page(&self) -> Element<'_, Message> {
        let benchmark =
            widget::settings::section()
                .title("Desempenho")
                .add(widget::text::caption(
                    "Os benchmarks usam o perfil padrão do nível selecionado.",
                ))
                .add(widget::button::text("Medir 40 mapas").on_press_maybe(
                    (!self.busy).then_some(Message::Benchmark(BenchmarkKind::Forty)),
                ))
                .add(
                    widget::button::text("Comparar lote de 800 mapas").on_press_maybe(
                        (!self.busy).then_some(Message::Benchmark(BenchmarkKind::Batch)),
                    ),
                )
                .add(widget::text::caption(&self.performance_status));
        widget::column::with_capacity(3)
            .push(
                widget::container(benchmark)
                    .padding([12, 20])
                    .width(Length::Fill),
            )
            .push(
                widget::container(result_panel(self.generated.as_ref(), self.level, self.seed))
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.nav_model.activate(id);
        Task::none()
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        let mut levels = widget::row::with_capacity(6)
            .spacing(4)
            .align_y(Alignment::Center)
            .push(widget::text::caption("Nível"));
        for level in 1..=5 {
            let button = if level == self.level {
                widget::button::suggested(level.to_string())
            } else {
                widget::button::standard(level.to_string())
            };
            levels = levels
                .push(button.on_press_maybe((!self.busy).then_some(Message::SelectLevel(level))));
        }
        vec![levels.into()]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        let seed_input = widget::text_input::text_input("Semente", self.seed_draft.clone());
        let seed_input = if self.busy {
            seed_input
        } else {
            seed_input
                .on_input(Message::SeedDraft)
                .on_submit(Message::SetSeed)
        };
        vec![
            widget::row::with_capacity(5)
                .spacing(4)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                        .on_press_maybe((!self.busy).then_some(Message::ChangeSeed(-1))),
                )
                .push(seed_input.width(Length::Fixed(96.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                        .on_press_maybe((!self.busy).then_some(Message::ChangeSeed(1))),
                )
                .push(widget::tooltip(
                    widget::button::icon(widget::icon::from_name(
                        "media-playlist-shuffle-symbolic",
                    ))
                    .on_press_maybe((!self.busy).then_some(Message::RandomSeed)),
                    widget::text::body("Nova semente aleatória (Alt+R)"),
                    widget::tooltip::Position::Bottom,
                ))
                .into(),
            widget::button::suggested(if self.busy { "Gerando…" } else { "Gerar" })
                .on_press_maybe((!self.busy).then_some(Message::Generate))
                .into(),
        ]
    }

    fn footer(&self) -> Option<Element<'_, Self::Message>> {
        Some(
            widget::container(
                widget::row::with_capacity(2)
                    .push(widget::text::caption(&self.status).width(Length::Fill))
                    .push(widget::text::caption(format!(
                        "Motor Rust · Rayon · {} threads",
                        par::threads()
                    )))
                    .align_y(Alignment::Center),
            )
            .padding([6, 14])
            .height(Length::Fixed(34.0))
            .width(Length::Fill)
            .into(),
        )
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        let keyboard = cosmic::iced::keyboard::listen().filter_map(|event| {
            let cosmic::iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                repeat,
                ..
            } = event
            else {
                return None;
            };
            // Plain letters, digits and arrows belong to the seed/parameter editors.
            if repeat || !modifiers.alt() {
                return None;
            }
            match key.as_ref() {
                cosmic::iced::keyboard::Key::Character("1") => Some(Message::KeyboardLevel(1)),
                cosmic::iced::keyboard::Key::Character("2") => Some(Message::KeyboardLevel(2)),
                cosmic::iced::keyboard::Key::Character("3") => Some(Message::KeyboardLevel(3)),
                cosmic::iced::keyboard::Key::Character("4") => Some(Message::KeyboardLevel(4)),
                cosmic::iced::keyboard::Key::Character("5") => Some(Message::KeyboardLevel(5)),
                cosmic::iced::keyboard::Key::Character("r" | "R") => {
                    Some(Message::KeyboardRandomSeed)
                }
                cosmic::iced::keyboard::Key::Named(
                    cosmic::iced::keyboard::key::Named::ArrowLeft,
                ) => Some(Message::KeyboardSeed(-1)),
                cosmic::iced::keyboard::Key::Named(
                    cosmic::iced::keyboard::key::Named::ArrowRight,
                ) => Some(Message::KeyboardSeed(1)),
                _ => None,
            }
        });
        let mouse_release = if self.map_dragging {
            cosmic::iced::event::listen().filter_map(|event| match event {
                cosmic::iced::Event::Mouse(cosmic::iced::mouse::Event::ButtonReleased(
                    cosmic::iced::mouse::Button::Left,
                )) => Some(Message::MapReleased),
                _ => None,
            })
        } else {
            cosmic::iced::Subscription::none()
        };
        cosmic::iced::Subscription::batch([keyboard, mouse_release])
    }

    fn init(
        mut core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Keep the map in the foreground; the header button opens navigation when needed.
        core.nav_bar_set_toggled(false);
        let mut nav_model = nav_bar::Model::default();
        nav_model
            .insert()
            .text("Mapa")
            .icon(widget::icon::from_name("view-grid-symbolic").icon())
            .data(StudioPage::Map);
        nav_model
            .insert()
            .text("Parâmetros")
            .icon(widget::icon::from_name("preferences-system-symbolic").icon())
            .data(StudioPage::Parameters);
        nav_model
            .insert()
            .text("Análise")
            .icon(widget::icon::from_name("view-statistics-symbolic").icon())
            .data(StudioPage::Analysis);
        nav_model.activate_position(0);
        let initial_params = params_of_level(1);
        let params_json = serde_json::to_string_pretty(&initial_params).unwrap_or_default();
        let open_groups = params_ui::groups()
            .into_iter()
            .filter(|group| group.open_by_default)
            .map(|group| group.id)
            .collect();
        let mut app = Self {
            core,
            nav_model,
            level: 1,
            seed: 1,
            seed_draft: "1".into(),
            show_params: false,
            params_editor: widget::text_editor::Content::with_text(&params_json),
            params: initial_params.clone(),
            defaults: initial_params,
            field_drafts: HashMap::new(),
            open_groups,
            changed_count: 0,
            overlays: Overlays {
                cells: true,
                labels: true,
                ..Overlays::default()
            },
            zoom: 1.0,
            pan: [0.0, 0.0],
            map_size: Size::new(800.0, 600.0),
            map_dragging: false,
            last_pointer: None,
            hover_info: String::new(),
            busy: false,
            work_id: 0,
            pending_revision: 0,
            status: "Preparando o gerador…".into(),
            performance_status: "Os benchmarks usam os parâmetros padrão do nível selecionado."
                .into(),
            generated: None,
            building: Arc::new(RwLock::new(BuildingMap::default())),
            map_cache: cosmic::iced::widget::canvas::Cache::default(),
            map_palette: RefCell::new(None),
            groups: params_ui::groups(),
        };
        app.set_header_title("Dungeon Lab".into());
        app.refresh_changed_count();
        let task = app.launch_generate();
        (app, task)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        match self.active_page() {
            StudioPage::Map => self.map_page(),
            StudioPage::Parameters => self.parameters_page(),
            StudioPage::Analysis => self.analysis_page(),
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        // Hovering and typing leave the map as it is; anything else may change it.
        let keeps_map = match &message {
            Message::MapPointerMoved { .. } | Message::MapReleased => !self.map_dragging,
            Message::SeedDraft(_)
            | Message::ParamDraft(..)
            | Message::ParamsEdited(_)
            | Message::ToggleGroup(_)
            | Message::CopyParams
            | Message::ToggleParams
            | Message::Benchmark(_)
            | Message::BenchmarkFinished(..) => true,
            _ => false,
        };
        if !keeps_map {
            self.map_cache.clear();
        }
        match message {
            Message::Generate => {
                if self.busy {
                    return Task::none();
                }
                if let Err(error) = self.commit_drafts() {
                    self.status = error;
                    return Task::none();
                }
                self.launch_generate()
            }
            Message::SelectLevel(level) => {
                if self.busy {
                    return Task::none();
                }
                self.current_level(level)
            }
            Message::SeedDraft(value) => {
                self.seed_draft = value;
                Task::none()
            }
            Message::SetSeed(value) => {
                if self.busy {
                    return Task::none();
                }
                match value.parse::<u32>() {
                    Ok(seed) => {
                        self.seed = seed;
                        self.seed_draft = seed.to_string();
                        self.status = "Semente atualizada.".into();
                        self.launch_generate()
                    }
                    Err(_) => {
                        self.seed_draft = self.seed.to_string();
                        self.status =
                            "A semente deve ser um inteiro entre 0 e 4.294.967.295.".into();
                        Task::none()
                    }
                }
            }
            Message::ChangeSeed(delta) => {
                if self.busy {
                    return Task::none();
                }
                self.seed = self.seed.saturating_add_signed(delta);
                self.seed_draft = self.seed.to_string();
                self.launch_generate()
            }
            Message::RandomSeed => {
                if self.busy {
                    return Task::none();
                }
                self.seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_nanos() as u32)
                    .unwrap_or_default();
                self.seed_draft = self.seed.to_string();
                self.launch_generate()
            }
            Message::ToggleParams => {
                self.show_params = !self.show_params;
                Task::none()
            }
            Message::ParamsEdited(action) => {
                if self.busy {
                    return Task::none();
                }
                self.params_editor.perform(action);
                self.field_drafts.clear();
                self.status = "Parâmetros alterados; validando antes de regenerar…".into();
                self.schedule_apply()
            }
            Message::ParamDraft(path, value) => {
                if self.busy {
                    return Task::none();
                }
                self.field_drafts.insert(path, value);
                self.status = "Parâmetros alterados; atualizando o mapa…".into();
                self.schedule_apply()
            }
            Message::ParamValue(path, value) => {
                if self.busy {
                    return Task::none();
                }
                self.change_param(path, value);
                self.schedule_apply()
            }
            Message::CycleChoice(path) => {
                if self.busy {
                    return Task::none();
                }
                if let Some(field) = params_ui::field(&path) {
                    if let FieldKind::Choice(options) = field.kind {
                        let current = get_path(
                            &serde_json::to_value(&self.params).unwrap_or(Value::Null),
                            &path,
                        )
                        .map(json_value_text)
                        .unwrap_or_default();
                        let next = options
                            .iter()
                            .position(|(value, _)| value == &current)
                            .map(|index| (index + 1) % options.len())
                            .unwrap_or(0);
                        if let Some((value, _)) = options.get(next) {
                            let param_value =
                                if matches!(path.as_str(), "m" | "discPorts" | "maxBends") {
                                    value.parse::<i64>().map(Value::from).unwrap_or(Value::Null)
                                } else {
                                    Value::String(value.clone())
                                };
                            self.change_param(path, param_value);
                        }
                    }
                }
                self.schedule_apply()
            }
            Message::ToggleGroup(id) => {
                if !self.open_groups.remove(&id) {
                    self.open_groups.insert(id);
                }
                Task::none()
            }
            Message::ResetGroup(id) => {
                if self.busy {
                    return Task::none();
                }
                self.reset_group(&id);
                self.schedule_apply()
            }
            Message::ResetAll => {
                if self.busy {
                    return Task::none();
                }
                self.current_level(self.level)
            }
            Message::ApplyPending(revision) => {
                if revision != self.pending_revision || self.busy {
                    return Task::none();
                }
                match self.commit_drafts() {
                    Ok(()) => self.launch_generate(),
                    Err(error) => {
                        self.status = error;
                        Task::none()
                    }
                }
            }
            Message::ToggleOverlay(key, enabled) => {
                match key.as_str() {
                    "cells" => self.overlays.cells = enabled,
                    "labels" => self.overlays.labels = enabled,
                    "ports" => self.overlays.ports = enabled,
                    "pairs" => self.overlays.pairs = enabled,
                    "candidates" => self.overlays.candidates = enabled,
                    _ => {}
                }
                Task::none()
            }
            Message::Preset(preset) => {
                if self.busy {
                    return Task::none();
                }
                self.set_preset(preset);
                self.launch_generate()
            }
            Message::CopyParams => {
                let export =
                    json!({ "level": self.level, "seed": self.seed, "params": self.params });
                self.status = "Parâmetros copiados em JSON.".into();
                cosmic::iced::clipboard::write::<cosmic::Action<Message>>(
                    serde_json::to_string_pretty(&export).unwrap_or_default(),
                )
            }
            Message::Benchmark(kind) => self.start_benchmark(kind),
            Message::BenchmarkFinished(id, result) => {
                if id == self.work_id {
                    self.busy = false;
                    self.performance_status = result;
                    self.status =
                        format!("Motor nativo · {} threads no pool Rayon", par::threads());
                }
                Task::none()
            }
            Message::KeyboardLevel(level) => {
                if self.busy {
                    Task::none()
                } else {
                    self.current_level(level)
                }
            }
            Message::KeyboardSeed(delta) => {
                if self.busy {
                    Task::none()
                } else {
                    self.seed = self.seed.saturating_add_signed(delta);
                    self.seed_draft = self.seed.to_string();
                    self.launch_generate()
                }
            }
            Message::KeyboardRandomSeed => {
                if self.busy {
                    Task::none()
                } else {
                    self.seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|duration| duration.as_nanos() as u32)
                        .unwrap_or_default();
                    self.seed_draft = self.seed.to_string();
                    self.launch_generate()
                }
            }
            Message::Zoom(factor) => self.zoom_map(factor, self.last_pointer, self.map_size),
            Message::FitMap => {
                self.zoom = 1.0;
                self.pan = [0.0, 0.0];
                Task::none()
            }
            Message::MapPressed => {
                self.map_dragging = true;
                self.hover_info.clear();
                Task::none()
            }
            Message::MapReleased => {
                self.map_dragging = false;
                Task::none()
            }
            Message::MapPointerMoved { point, size } => {
                self.map_size = size;
                let mut panned = false;
                if self.map_dragging {
                    if let Some(last) = self.last_pointer {
                        if let Some((min, max)) = self.map_bounds() {
                            let (_, _, width, height) = view_box(min, max, self.zoom, self.pan);
                            let scale = (size.width / width as f32)
                                .min(size.height / height as f32)
                                .max(0.001);
                            self.pan[0] -= (point.x - last.x) as f64 / scale as f64;
                            self.pan[1] -= (point.y - last.y) as f64 / scale as f64;
                            panned = true;
                        }
                    }
                } else if let Some(map) = &self.generated {
                    if let Some(world) = self.map_bounds().and_then(|(min, max)| {
                        pointer_world(min, max, self.zoom, self.pan, point, size)
                    }) {
                        self.hover_info = map_object_at(&map.dungeon, world, self.zoom);
                    }
                }
                if panned {
                    self.last_pointer = Some(point);
                    return Task::none();
                }
                self.last_pointer = Some(point);
                Task::none()
            }
            Message::MapScrolled(delta) => {
                let y = match delta {
                    ScrollDelta::Lines { y, .. } => y * 40.0,
                    ScrollDelta::Pixels { y, .. } => y,
                };
                let factor = (-f64::from(y) * 0.0012).exp();
                self.zoom_map(factor, self.last_pointer, self.map_size)
            }
            Message::Generated(id, result) => {
                if id != self.work_id {
                    return Task::none();
                }
                self.busy = false;
                match result {
                    Ok(map) => {
                        self.generated = Some(map);
                        self.building = Arc::new(RwLock::new(BuildingMap::default()));
                        self.hover_info.clear();
                        self.status = format!(
                            "Andar gerado em {:.1} ms · {} tentativas · {} threads disponíveis.",
                            self.generated.as_ref().map_or(0.0, |map| map.elapsed_ms),
                            self.generated.as_ref().map_or(0, |map| map.draws),
                            par::threads()
                        );
                    }
                    Err(error) => self.status = format!("Falha na geração: {error}"),
                }
                Task::none()
            }
            Message::GenerationProgress(id, events) => {
                if id != self.work_id {
                    return Task::none();
                }
                for event in events {
                    match event {
                        GenerationEvent::AttemptStarted { draw, seed } => {
                            self.building = Arc::new(RwLock::new(BuildingMap {
                                draw,
                                seed,
                                ..BuildingMap::default()
                            }));
                            self.generated = None;
                            self.hover_info.clear();
                            self.status = format!("Tentativa {draw}: montando o mapa…");
                        }
                        GenerationEvent::Layout {
                            grid_x,
                            grid_y,
                            corridor_width,
                        } => {
                            if let Ok(mut building) = self.building.write() {
                                building.grid_x = grid_x;
                                building.grid_y = grid_y;
                                building.corridor_width = corridor_width;
                                self.status = format!(
                                    "Tentativa {} · semente {} · grade pronta · colocando salas…",
                                    building.draw, building.seed
                                );
                            }
                        }
                        GenerationEvent::RoomAdded(shape) => {
                            if let Ok(mut building) = self.building.write() {
                                building.rooms.push(shape);
                                self.status = format!(
                                    "Tentativa {} · {} salas · {} conexões",
                                    building.draw,
                                    building.rooms.len(),
                                    building.connections
                                );
                            }
                        }
                        GenerationEvent::EdgeUpdated { id, pts } => {
                            if let Ok(mut building) = self.building.write() {
                                if building.edges.len() <= id {
                                    building.edges.resize_with(id + 1, Vec::new);
                                }
                                if building.edges[id].is_empty() {
                                    building.connections += 1;
                                }
                                building.edges[id] = pts;
                                self.status = format!(
                                    "Tentativa {} · {} salas · {} conexões",
                                    building.draw,
                                    building.rooms.len(),
                                    building.connections
                                );
                            }
                        }
                    }
                }
                Task::none()
            }
        }
    }
}

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(960.0)
            .min_height(640.0),
    );
    cosmic::app::run::<App>(settings, ())
}

fn level_description(level: usize) -> &'static str {
    match level {
        1 => "Introdução: salas retangulares e poucas conexões extras.",
        2 => "Acrescenta salas em L e as primeiras junções.",
        3 => "Amplia o mapa e libera diagonais e salas circulares.",
        4 => "Combina mais formas, junções e portas na escada.",
        _ => "Configuração mais rica: mais salas, formas e alternativas de conexão.",
    }
}

fn param_field_view(field: ParamField, value: String, busy: bool) -> Element<'static, Message> {
    let help = widget::tooltip(
        widget::button::text("ⓘ"),
        widget::text::body(field.help.clone()),
        widget::tooltip::Position::Right,
    );
    let label = widget::text::caption(field.label.clone());
    let is_boolean = matches!(&field.kind, FieldKind::Boolean);
    let control: Element<'static, Message> = match field.kind {
        FieldKind::Number { min, max, integer } => {
            let path = field.path.clone();
            let input = widget::text_input::text_input(
                if integer {
                    format!("{min:.0}–{max:.0}")
                } else {
                    format!("{min}–{max}")
                },
                value,
            );
            let input = if busy {
                input
            } else {
                input.on_input(move |value| Message::ParamDraft(path.clone(), value))
            };
            input.width(Length::Fixed(100.0)).into()
        }
        FieldKind::Boolean => {
            let enabled = value == "true";
            let path = field.path.clone();
            widget::toggler(enabled)
                .label(Some(field.label.clone()))
                .on_toggle_maybe(
                    (!busy).then_some(move |value| Message::ParamValue(path.clone(), json!(value))),
                )
                .into()
        }
        FieldKind::Choice(options) => {
            let selected = options
                .iter()
                .find(|(choice, _)| choice == &value)
                .map(|(_, label)| label.as_str())
                .unwrap_or(value.as_str());
            widget::button::text(selected.to_string())
                .on_press_maybe((!busy).then_some(Message::CycleChoice(field.path.clone())))
                .into()
        }
    };
    let row = widget::row::with_capacity(3)
        .spacing(6)
        .align_y(Alignment::Center);
    let row = if is_boolean {
        row.push(control).push(help)
    } else {
        row.push(label.width(Length::Fixed(185.0)))
            .push(help)
            .push(control)
    };
    widget::container(row).padding([4, 2]).into()
}

fn result_panel(
    generated: Option<&GeneratedMap>,
    level: usize,
    seed: u32,
) -> Element<'static, Message> {
    let Some(map) = generated else {
        return widget::container(widget::text::body(
            "As métricas e a validação aparecem aqui.",
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .into();
    };
    let m = &map.metrics;
    let mut panel = widget::column::with_capacity(8)
        .push(widget::text::title3("Leitura do andar"))
        .push(widget::text::caption(format!(
            "Nível {level} · semente {seed}"
        )));
    if map.violations.is_empty() {
        panel = panel.push(widget::text::title4("✓ Mapa válido"));
    } else {
        panel = panel
            .push(widget::text::title4(format!(
                "! {} problema(s)",
                map.violations.len()
            )))
            .push(widget::text::caption(format!(
                "Após {} tentativa(s), o mapa ainda não atende a todas as regras.",
                map.draws
            )));
        for violation in map.violations.iter().take(30) {
            panel = panel.push(widget::text::caption(translate_violation(violation)));
        }
    }
    panel = panel.push(widget::text::caption(format!(
        "{} tentativa(s) · geração {:.1} ms",
        map.draws, map.elapsed_ms
    )));
    panel = panel.push(widget::text::caption(map.summary.clone()));

    panel = panel.push(widget::text::title4("Tamanho do mapa"));
    for (label, value, help) in [
        (
            "Salas principais",
            m.rooms.to_string(),
            "Salas ativas, sem contar câmaras-folha.",
        ),
        (
            "Câmaras-folha",
            m.leaf_chambers.to_string(),
            "Salas extras ligadas por um único corredor.",
        ),
        (
            "Junções T/X",
            m.junctions.to_string(),
            "Pontos onde três ou quatro braços se encontram.",
        ),
        (
            "Trechos de corredor",
            m.edges.to_string(),
            "Arestas do grafo; cada braço de junção conta como trecho.",
        ),
    ] {
        panel = panel.push(metric_line(label, &value, help));
    }

    panel = panel.push(widget::text::title4("Ciclos e densidade"));
    for (label, value, help) in [
        (
            "Ciclos atuais β",
            m.beta.to_string(),
            "Número de ciclos independentes; uma árvore tem zero.",
        ),
        (
            "Alvo de ciclos",
            m.beta_target.to_string(),
            "Alvo calculado a partir da densidade e da referência do gerador.",
        ),
        (
            "Referência βmax",
            m.beta_max.to_string(),
            "Estimativa gulosa dos ciclos que cabem, incluindo junções.",
        ),
        (
            "Densidade δ",
            format!("{:.2}", m.delta),
            "Proporção entre ciclos atuais e a referência βmax.",
        ),
    ] {
        panel = panel.push(metric_line(label, &value, help));
    }

    panel = panel.push(widget::text::title4("Caminhos e conectividade"));
    let routes = [
        (
            "Portas usadas por sala",
            format!("{:.2}", m.mean_degree),
            "Média de corredores ligados a cada sala.",
        ),
        (
            "Salas de uma porta",
            m.leaves.to_string(),
            "Salas com apenas uma conexão; incluem becos, entrada e escada.",
        ),
        (
            "Pontes",
            format!("{} · {:.0}%", m.bridges, m.bridge_share * 100.0),
            "Trechos cuja remoção desconectaria uma parte do mapa.",
        ),
        (
            "Salas de articulação",
            m.articulations.to_string(),
            "Salas cuja remoção dividiria o grafo.",
        ),
        (
            "Passos até a escada",
            m.hops_to_stairs
                .map(|value| format!("{value:.0}"))
                .unwrap_or_else(|| "—".into()),
            "Número de salas no caminho mais curto da entrada à escada.",
        ),
        (
            "Diâmetro",
            format!("{:.0}", m.diameter),
            "Maior distância mínima entre duas salas, em passos.",
        ),
        (
            "Rotas independentes até a escada",
            m.routes_to_stairs.to_string(),
            "Caminhos entre entrada e escada que não compartilham corredores.",
        ),
        (
            "Menor ciclo",
            m.cycle_girth
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".into()),
            "Número de trechos no menor ciclo do mapa.",
        ),
        (
            "Ramos laterais",
            m.side_branches.to_string(),
            "Componentes opcionais presos ao caminho principal por uma passagem.",
        ),
        (
            "Profundidade dos ramos",
            m.deepest_branch.to_string(),
            "Maior número de salas em um ramo lateral.",
        ),
        (
            "Maior grau",
            m.max_node_degree.to_string(),
            "Maior quantidade de braços ligados a um mesmo ponto.",
        ),
        (
            "Caminho geométrico",
            format!("{:.0} px", m.geometric_path_to_stairs),
            "Comprimento físico do menor caminho até a escada.",
        ),
        (
            "Conectividade λ₂",
            format!("{:.3}", m.lambda2),
            "Valores maiores tendem a indicar menor dependência de gargalos.",
        ),
    ];
    for (label, value, help) in routes {
        panel = panel.push(metric_line(label, &value, help));
    }

    panel = panel.push(widget::text::title4("Formas no mapa"));
    for (class, count) in &m.by_class {
        panel = panel.push(widget::text::caption(format!(
            "{}: {count}",
            class_label(class)
        )));
    }
    if !map.dungeon.log.is_empty() {
        panel = panel.push(widget::text::title4("O que o gerador fez"));
        for line in &map.dungeon.log {
            panel = panel.push(widget::text::caption(translate_log(line)));
        }
    }
    panel = panel.push(widget::text::title4("Legenda"));
    panel = panel.push(widget::text::caption(
        "As cores das salas indicam suas formas; o contorno indica o papel. Portões aparecem tracejados em dourado, portas usadas em branco e junções como pontos brancos.",
    ));
    widget::container(widget::scrollable(panel.spacing(7).padding(14)))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn metric_line(label: &str, value: &str, help: &str) -> Element<'static, Message> {
    let tooltip = widget::tooltip(
        widget::button::text("ⓘ"),
        widget::text::body(help.to_string()),
        widget::tooltip::Position::Left,
    );
    widget::row::with_capacity(3)
        .push(widget::text::caption(label.to_string()).width(Length::Fill))
        .push(tooltip)
        .push(widget::text::body(value.to_string()))
        .spacing(3)
        .align_y(Alignment::Center)
        .into()
}

fn translate_violation(message: &str) -> String {
    if let Some((code, detail)) = message.split_once(' ') {
        let meaning = match code {
            "V1" => "alcance das salas",
            "V2" => "distância e colisões",
            "V3" => "portas",
            "V4" => "geometria dos corredores",
            "V5" => "entrada e escada",
            "T1" | "T2" | "T3" | "T4" | "T5" => "requisito da topologia",
            "P1" | "P2" | "P3" => "progressão",
            _ => "regra do modelo",
        };
        format!("{meaning}: {detail}")
    } else {
        message.to_string()
    }
}

fn translate_log(message: &str) -> String {
    if let Some(value) = message.strip_prefix("tree stuck: near ") {
        return format!("A busca ampliou o raio de conexão para {value}.");
    }
    if let Some(value) = message.strip_prefix("layout jitter limited to ") {
        return format!("A irregularidade da grade foi limitada a {value}.");
    }
    message.to_string()
}

fn class_label(class: &str) -> &str {
    match class {
        "rect" => "Retangulares",
        "disc" => "Circulares",
        "oct" => "Octogonais",
        "L" => "Em L",
        "star" => "Estrelas truncadas",
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

fn format_number(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
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

fn pointer_world(
    min: [f64; 2],
    max: [f64; 2],
    zoom: f64,
    pan: [f64; 2],
    point: Point,
    size: Size,
) -> Option<[f64; 2]> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    let (x, y, width, height) = view_box(min, max, zoom, pan);
    let scale = (size.width as f64 / width).min(size.height as f64 / height);
    let drawn_width = width * scale;
    let drawn_height = height * scale;
    let offset_x = (size.width as f64 - drawn_width) / 2.0;
    let offset_y = (size.height as f64 - drawn_height) / 2.0;
    Some([
        x + (point.x as f64 - offset_x) / scale,
        y + (point.y as f64 - offset_y) / scale,
    ])
}

fn run_benchmark(level: usize, kind: BenchmarkKind) -> String {
    let params = params_of_level(level);
    match kind {
        BenchmarkKind::Forty => {
            let started = Instant::now();
            let mut invalid = 0usize;
            for seed in 1..=40u32 {
                invalid += usize::from(!stage::stage(seed, &params, 8).violations.is_empty());
            }
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            format!(
                "Média: {:.2} ms por mapa · {invalid} inválido(s) de 40 · {} threads disponíveis.",
                elapsed / 40.0,
                par::threads()
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
                "800 mapas: 1 thread {:.0} ms · {} threads {:.0} ms · inválidos: {one_invalid}/{parallel_invalid}.",
                one_ms,
                par::threads(),
                parallel_ms
            )
        }
    }
}

/// Longest delay between preview events, in ms. Rooms and corridor edits appear one at a time.
const PREVIEW_STEP_MS: u64 = 35;
/// Target total preview time per attempt, in ms: large grids get a shorter step so the wait stays bounded.
const PREVIEW_BUDGET_MS: u64 = 6000;
static PREVIEW_PAUSED_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn generate_map(
    level: usize,
    seed: u32,
    params: Params,
    mut progress: impl FnMut(GenerationEvent),
) -> Result<GeneratedMap, String> {
    let started = Instant::now();
    PREVIEW_PAUSED_NS.store(0, std::sync::atomic::Ordering::Relaxed);
    let stage = stage::stage_with_progress(seed, &params, 8, &mut progress);
    let paused_ms = PREVIEW_PAUSED_NS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6;
    let elapsed_ms = (started.elapsed().as_secs_f64() * 1000.0 - paused_ms).max(0.0);
    let measures = metrics_of(&stage.d);
    let chosen_seed = stage.d.seed;
    let phases = stage
        .d
        .phases
        .iter()
        .map(|(name, ms)| format!("{name} {ms:.1} ms"))
        .collect::<Vec<_>>()
        .join(" · ");
    let validity = if stage.violations.is_empty() {
        "válido"
    } else {
        "com violações"
    };
    let summary = format!(
        "Nível {level} · seed {chosen_seed} · {validity} · geração de um andar em {elapsed_ms:.1} ms · {} tentativa(s)\n{} salas · {} corredores · β={} · δ={:.2} · {} rotas até a saída\n{phases}{}",
        stage.draws,
        measures.rooms,
        measures.edges,
        measures.beta,
        measures.delta,
        measures.routes_to_stairs,
        if stage.violations.is_empty() { String::new() } else { format!("\nAvisos: {}", stage.violations.join("; ")) },
    );

    Ok(GeneratedMap {
        dungeon: Arc::new(stage.d),
        metrics: measures,
        violations: stage.violations,
        draws: stage.draws,
        elapsed_ms,
        summary,
    })
}

fn validate_params(p: &Params) -> Result<(), String> {
    fn float(name: &str, value: f64, min: f64, max: f64) -> Result<(), String> {
        if value.is_finite() && (min..=max).contains(&value) {
            Ok(())
        } else {
            Err(format!("{name} deve estar entre {min} e {max}."))
        }
    }

    fn int(name: &str, value: i32, min: i32, max: i32) -> Result<(), String> {
        if (min..=max).contains(&value) {
            Ok(())
        } else {
            Err(format!("{name} deve estar entre {min} e {max}."))
        }
    }

    fn count(name: &str, value: usize, max: usize) -> Result<(), String> {
        if value <= max {
            Ok(())
        } else {
            Err(format!("{name} deve estar entre 0 e {max}."))
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
        return Err("m deve ser 4 ou 8.".into());
    }
    if !matches!(p.disc_ports, 4 | 8 | 16) {
        return Err("discPorts deve ser 4, 8 ou 16.".into());
    }
    if !matches!(p.max_bends, 0 | 1 | 2 | 4 | 5 | 6) {
        return Err("maxBends deve ser 0, 1, 2, 4, 5 ou 6.".into());
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
            return Err(format!("maxOf.{class} deve ser um número inteiro."));
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
    for (name, value) in [
        ("leafRoles.trap", p.leaf_roles.trap),
        ("leafRoles.prize", p.leaf_roles.prize),
        ("leafRoles.empty", p.leaf_roles.empty),
    ] {
        float(name, value, 0.0, 1000.0)?;
    }
    for (name, value) in &p.mix {
        float(&format!("mix.{name}"), *value, 0.0, 1.0)?;
    }
    if let Some(value) = p.junction {
        float("junction", value, 0.0, 10000.0)?;
    }
    int("stairsDoors", p.stairs_doors, 1, 1000)?;
    int("minHops", p.min_hops, 1, 1000)?;
    int("ambush", p.ambush, 0, 1000)?;
    int("leaves", p.leaves, 0, 1000)?;
    count("topology.minBranches", p.topology.min_branches, 1000)?;
    count("topology.minBranchDepth", p.topology.min_branch_depth, 1000)?;
    count("topology.minExitRoutes", p.topology.min_exit_routes, 1000)?;
    count("topology.minCycleLength", p.topology.min_cycle_length, 1000)?;
    count("topology.minHubDegree", p.topology.min_hub_degree, 1000)?;
    count("progression.keyLocks", p.progression.key_locks, 8)?;
    count("progression.switchLocks", p.progression.switch_locks, 8)?;
    Ok(())
}

#[derive(Clone)]
struct MapPalette {
    background: Color,
    surface: Color,
    grid: Color,
    accent: Color,
    success: Color,
    warning: Color,
    destructive: Color,
    foreground: Color,
    is_dark: bool,
}

impl MapPalette {
    fn current() -> Self {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let background = cosmic.background(theme.transparent);
        Self {
            background: background.base.into(),
            surface: background.component.base.into(),
            grid: background.divider.into(),
            accent: cosmic.accent.base.into(),
            success: cosmic.success.base.into(),
            warning: cosmic.warning.base.into(),
            destructive: cosmic.destructive.base.into(),
            foreground: background.on.into(),
            is_dark: cosmic.is_dark,
        }
    }
}

struct MapCanvas<'a> {
    dungeon: Option<Arc<Dungeon>>,
    building: Arc<RwLock<BuildingMap>>,
    overlays: Overlays,
    zoom: f64,
    pan: [f64; 2],
    palette: MapPalette,
    cache: &'a cosmic::iced::widget::canvas::Cache,
    /// While the map is dragged the detail layers are dropped, so each frame stays cheap.
    dragging: bool,
}

impl<'a> cosmic::iced::widget::canvas::Program<Message, cosmic::Theme, cosmic::Renderer>
    for MapCanvas<'a>
{
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: cosmic::iced::mouse::Cursor,
    ) -> Vec<cosmic::iced::widget::canvas::Geometry> {
        let overlays = if self.dragging {
            Overlays {
                labels: false,
                ports: false,
                pairs: false,
                candidates: false,
                ..self.overlays
            }
        } else {
            self.overlays
        };
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.palette.background);
            let Ok(building) = self.building.read() else {
                return;
            };
            let extents = self
                .dungeon
                .as_deref()
                .map(|dungeon| {
                    (
                        [dungeon.grid_x[0], dungeon.grid_y[0]],
                        [
                            *dungeon.grid_x.last().unwrap(),
                            *dungeon.grid_y.last().unwrap(),
                        ],
                    )
                })
                .or_else(|| building.bounds());
            let Some((min, max)) = extents else {
                return;
            };
            let (x, y, width, height) = view_box(min, max, self.zoom, self.pan);
            let scale = (bounds.width as f64 / width).min(bounds.height as f64 / height);
            let offset_x = (bounds.width as f64 - width * scale) / 2.0;
            let offset_y = (bounds.height as f64 - height * scale) / 2.0;
            frame.with_save(|frame| {
                frame.translate(Vector::new(offset_x as f32, offset_y as f32));
                frame.scale(scale as f32);
                frame.translate(Vector::new(-x as f32, -y as f32));
                if overlays.cells {
                    if let Some(dungeon) = self.dungeon.as_deref() {
                        draw_grid(frame, &dungeon.grid_x, &dungeon.grid_y, self.palette.grid);
                    } else {
                        draw_grid(frame, &building.grid_x, &building.grid_y, self.palette.grid);
                    }
                }
                if let Some(dungeon) = self.dungeon.as_deref() {
                    draw_dungeon(frame, dungeon, overlays, &self.palette);
                } else {
                    draw_building(frame, &building, &self.palette);
                }
            });
        });
        vec![geometry]
    }
}

fn draw_grid(
    frame: &mut cosmic::iced::widget::canvas::Frame,
    grid_x: &[f64],
    grid_y: &[f64],
    color: Color,
) {
    use cosmic::iced::widget::canvas::{self, Path};
    if grid_x.is_empty() || grid_y.is_empty() {
        return;
    }
    let path = Path::new(|path| {
        for x in grid_x {
            path.move_to(Point::new(*x as f32, grid_y[0] as f32));
            path.line_to(Point::new(*x as f32, *grid_y.last().unwrap() as f32));
        }
        for y in grid_y {
            path.move_to(Point::new(grid_x[0] as f32, *y as f32));
            path.line_to(Point::new(*grid_x.last().unwrap() as f32, *y as f32));
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color.scale_alpha(0.42))
            .with_width(2.0),
    );
}

fn draw_building(
    frame: &mut cosmic::iced::widget::canvas::Frame,
    building: &BuildingMap,
    palette: &MapPalette,
) {
    for points in &building.edges {
        if !points.is_empty() {
            draw_polyline(
                frame,
                points,
                palette.surface,
                building.corridor_width.max(2.0) as f32,
                false,
            );
        }
    }
    for shape in &building.rooms {
        draw_shape(
            frame,
            shape,
            Some(shape_color("rect", palette.is_dark).scale_alpha(0.9)),
            palette.grid,
            4.0,
            false,
        );
    }
}

fn draw_dungeon(
    frame: &mut cosmic::iced::widget::canvas::Frame,
    dungeon: &Dungeon,
    overlays: Overlays,
    palette: &MapPalette,
) {
    use cosmic::iced::widget::canvas::{self, Path};

    if overlays.pairs {
        let pairs = Path::new(|path| {
            for [a, b] in &dungeon.pairs {
                if let (Some(first), Some(second)) = (dungeon.rooms.get(*a), dungeon.rooms.get(*b))
                {
                    path.move_to(world_point(first.center));
                    path.line_to(world_point(second.center));
                }
            }
        });
        let mut stroke = canvas::Stroke::default()
            .with_color(palette.accent.scale_alpha(0.55))
            .with_width(3.0);
        stroke.line_dash = canvas::LineDash {
            offset: 0,
            segments: &[10.0, 9.0],
        };
        frame.stroke(&pairs, stroke);
    }
    if overlays.candidates {
        for candidate in &dungeon.candidates {
            draw_polyline(
                frame,
                &candidate.pts,
                palette.accent.scale_alpha(0.55),
                5.0,
                false,
            );
        }
    }
    for edge in &dungeon.edges {
        let gated = edge.requires_keys > 0 || edge.requires_switches > 0;
        draw_polyline(
            frame,
            &edge.pts,
            if gated {
                palette.warning
            } else {
                palette.surface
            },
            dungeon.params.width.max(2.0) as f32,
            gated,
        );
        if gated {
            let middle = point_along(&edge.pts, 0.5);
            let marker = Path::circle(world_point(middle), 13.0);
            frame.fill(&marker, palette.background);
            frame.stroke(
                &marker,
                canvas::Stroke::default()
                    .with_color(palette.warning)
                    .with_width(3.0),
            );
            draw_text(
                frame,
                if edge.requires_keys > 0 { "K" } else { "S" },
                middle,
                palette.foreground,
                14.0,
            );
        }
    }
    for room in &dungeon.rooms {
        if room.alive {
            draw_shape(
                frame,
                &room.shape,
                Some(
                    shape_color(room.cls.name(), palette.is_dark).scale_alpha(if room.leaf {
                        0.65
                    } else {
                        0.9
                    }),
                ),
                role_color(room.role, palette),
                4.0,
                false,
            );
        } else {
            draw_shape(frame, &room.shape, None, palette.destructive, 3.0, true);
        }
    }
    if overlays.ports {
        for room in &dungeon.rooms {
            for port in &room.ports {
                let marker = Path::rectangle(
                    Point::new((port.p[0] - 4.0) as f32, (port.p[1] - 4.0) as f32),
                    Size::new(8.0, 8.0),
                );
                frame.fill(&marker, palette.foreground.scale_alpha(0.52));
            }
        }
    }
    for room in &dungeon.rooms {
        for port in &room.used {
            let direction = dir_of(port.k);
            draw_polyline(
                frame,
                &[
                    port.p,
                    [
                        port.p[0] + direction[0] * 14.0,
                        port.p[1] + direction[1] * 14.0,
                    ],
                ],
                palette.foreground,
                5.0,
                false,
            );
        }
    }
    for node in &dungeon.nodes {
        if let Node::Junction { p } = node {
            let marker = Path::circle(world_point(**p), 7.0);
            frame.fill(&marker, palette.foreground);
            frame.stroke(
                &marker,
                canvas::Stroke::default()
                    .with_color(palette.background)
                    .with_width(2.0),
            );
        }
    }
    if overlays.labels {
        for room in dungeon.rooms.iter().filter(|room| room.alive) {
            let inventory = [
                (room.keys != 0).then(|| format!("K{}", room.keys.count_ones())),
                (room.switches != 0).then(|| format!("S{}", room.switches.count_ones())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
            let role = if room.role == Role::Plain {
                String::new()
            } else {
                format!(" · {}", role_label(room.role))
            };
            let label = format!(
                "#{} {}{}{}",
                room.id,
                class_label(room.cls.name()),
                role,
                if inventory.is_empty() {
                    String::new()
                } else {
                    format!(" · {inventory}")
                }
            );
            draw_text(frame, &label, room.center, palette.foreground, 18.0);
        }
    }
}

fn draw_polyline(
    frame: &mut cosmic::iced::widget::canvas::Frame,
    points: &[[f64; 2]],
    color: Color,
    width: f32,
    dashed: bool,
) {
    use cosmic::iced::widget::canvas::{self, Path};
    if points.len() < 2 {
        return;
    }
    let path = Path::new(|path| {
        path.move_to(world_point(points[0]));
        for point in &points[1..] {
            path.line_to(world_point(*point));
        }
    });
    let mut stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(canvas::LineCap::Round)
        .with_line_join(canvas::LineJoin::Round);
    if dashed {
        stroke.line_dash = canvas::LineDash {
            offset: 0,
            segments: &[14.0, 9.0],
        };
    }
    frame.stroke(&path, stroke);
}

fn draw_shape(
    frame: &mut cosmic::iced::widget::canvas::Frame,
    shape: &Shape,
    fill: Option<Color>,
    stroke: Color,
    width: f32,
    dashed: bool,
) {
    use cosmic::iced::widget::canvas::{self, Path};
    let path = match shape {
        Shape::Poly { pts } => Path::new(|path| {
            if let Some(first) = pts.first() {
                path.move_to(world_point(*first));
                for point in &pts[1..] {
                    path.line_to(world_point(*point));
                }
                path.close();
            }
        }),
        Shape::Disc { c, r } => Path::circle(world_point(*c), *r as f32),
    };
    if let Some(fill) = fill {
        frame.fill(&path, fill);
    }
    let mut style = canvas::Stroke::default()
        .with_color(stroke)
        .with_width(width);
    if dashed {
        style.line_dash = canvas::LineDash {
            offset: 0,
            segments: &[9.0, 8.0],
        };
    }
    frame.stroke(&path, style);
}

fn draw_text(
    frame: &mut cosmic::iced::widget::canvas::Frame,
    content: impl Into<String>,
    position: [f64; 2],
    color: Color,
    size: f32,
) {
    use cosmic::iced::widget::canvas;
    frame.fill_text(canvas::Text {
        content: content.into(),
        position: world_point(position),
        color,
        size: size.into(),
        align_x: cosmic::iced::widget::text::Alignment::Center,
        align_y: cosmic::iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

fn world_point(point: [f64; 2]) -> Point {
    Point::new(point[0] as f32, point[1] as f32)
}

fn point_along(points: &[[f64; 2]], fraction: f64) -> [f64; 2] {
    let lengths = points
        .windows(2)
        .map(|pair| dist(pair[0], pair[1]))
        .collect::<Vec<_>>();
    let mut remaining = lengths.iter().sum::<f64>() * fraction;
    for (index, length) in lengths.iter().enumerate() {
        if remaining <= *length {
            let t = if *length > 0.0 {
                remaining / length
            } else {
                0.0
            };
            return [
                points[index][0] + (points[index + 1][0] - points[index][0]) * t,
                points[index][1] + (points[index + 1][1] - points[index][1]) * t,
            ];
        }
        remaining -= length;
    }
    points.last().copied().unwrap_or([0.0, 0.0])
}

fn shape_color(class: &str, dark: bool) -> Color {
    match (class, dark) {
        ("rect", true) => Color::from_rgb8(0x49, 0x6c, 0x9a),
        ("disc", true) => Color::from_rgb8(0x29, 0xa8, 0x9a),
        ("oct", true) => Color::from_rgb8(0x9b, 0x78, 0xd1),
        ("L", true) => Color::from_rgb8(0xce, 0x84, 0x3c),
        ("star", true) => Color::from_rgb8(0xd0, 0xae, 0x38),
        (_, true) => Color::from_rgb8(0x59, 0x67, 0x7a),
        ("rect", false) => Color::from_rgb8(0x31, 0x5f, 0x96),
        ("disc", false) => Color::from_rgb8(0x08, 0x7f, 0x76),
        ("oct", false) => Color::from_rgb8(0x72, 0x53, 0xaf),
        ("L", false) => Color::from_rgb8(0xa9, 0x58, 0x12),
        ("star", false) => Color::from_rgb8(0x93, 0x6c, 0x00),
        (_, false) => Color::from_rgb8(0x59, 0x67, 0x7a),
    }
}

fn role_color(role: Role, palette: &MapPalette) -> Color {
    match role {
        Role::Start | Role::Rest => palette.success,
        Role::Stairs | Role::Switch => palette.accent,
        Role::Ambush | Role::Boss | Role::Trap => palette.destructive,
        Role::Key | Role::Prize => palette.warning,
        Role::Empty | Role::Plain => palette.grid,
    }
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Start => "entrada",
        Role::Stairs => "escada",
        Role::Ambush => "emboscada",
        Role::Boss => "chefe",
        Role::Rest => "descanso",
        Role::Key => "chave",
        Role::Switch => "interruptor",
        Role::Plain => "comum",
        Role::Trap => "armadilha",
        Role::Prize => "prêmio",
        Role::Empty => "vazia",
    }
}

fn point_in_shape(shape: &Shape, point: [f64; 2]) -> bool {
    match shape {
        Shape::Disc { c, r } => dist(*c, point) <= *r,
        Shape::Poly { pts } => {
            let mut inside = false;
            let mut j = pts.len().saturating_sub(1);
            for i in 0..pts.len() {
                let [xi, yi] = pts[i];
                let [xj, yj] = pts[j];
                let crosses = (yi > point[1]) != (yj > point[1])
                    && point[0] < (xj - xi) * (point[1] - yi) / (yj - yi) + xi;
                if crosses {
                    inside = !inside;
                }
                j = i;
            }
            inside
        }
    }
}

fn distance_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length2 = dx * dx + dy * dy;
    let t = if length2 == 0.0 {
        0.0
    } else {
        (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2).clamp(0.0, 1.0)
    };
    dist(point, [a[0] + dx * t, a[1] + dy * t])
}

fn node_label(dungeon: &Dungeon, node: usize) -> String {
    match dungeon.nodes.get(node) {
        Some(Node::Junction { .. }) => "junção".into(),
        Some(Node::Room { room }) => format!("sala #{}", dungeon.rooms[*room].id),
        None => "ponto".into(),
    }
}

fn map_object_at(dungeon: &Dungeon, point: [f64; 2], zoom: f64) -> String {
    for room in dungeon.rooms.iter().rev() {
        if room.alive && point_in_shape(&room.shape, point) {
            let kind = if room.leaf { "câmara-folha" } else { "sala" };
            let items = [
                (room.keys != 0).then(|| format!("{} chave(s)", room.keys.count_ones())),
                (room.switches != 0)
                    .then(|| format!("{} interruptor(es)", room.switches.count_ones())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            return format!(
                "{} · {}\nSala #{} · {} · {}\nÁrea: {:.0} px²\nPortas usadas: {} · possíveis: {}{}",
                kind,
                role_label(room.role),
                room.id,
                class_label(room.cls.name()),
                role_label(room.role),
                room.area,
                room.used.len(),
                room.ports.len(),
                if items.is_empty() {
                    String::new()
                } else {
                    format!("\nItens: {}", items.join(" · "))
                }
            );
        }
    }
    for (id, node) in dungeon.nodes.iter().enumerate() {
        if let Node::Junction { p } = node {
            if dist(**p, point) <= (8.0 / zoom.max(0.1)).max(6.0) {
                let degree = dungeon
                    .edges
                    .iter()
                    .filter(|edge| edge.u == id || edge.v == id)
                    .count();
                return format!(
                    "Junção de corredor\n{degree} braços conectados\nPosição: {:.0}, {:.0} px",
                    p[0], p[1]
                );
            }
        }
    }
    for edge in &dungeon.edges {
        if edge.pts.windows(2).any(|segment| {
            distance_to_segment(point, segment[0], segment[1])
                <= dungeon.params.width / 2.0 + 5.0 / zoom.max(0.1)
        }) {
            let length = edge
                .pts
                .windows(2)
                .map(|segment| dist(segment[0], segment[1]))
                .sum::<f64>();
            let gates = [
                (edge.requires_keys != 0).then(|| format!("chave {:08b}", edge.requires_keys)),
                (edge.requires_switches != 0)
                    .then(|| format!("interruptor {:08b}", edge.requires_switches)),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            return format!(
                "Trecho de corredor #{}\n{} → {}\n{} segmento(s) · {:.0} px{}",
                edge.id,
                node_label(dungeon, edge.u),
                node_label(dungeon, edge.v),
                edge.pts.len().saturating_sub(1),
                length,
                if gates.is_empty() {
                    String::new()
                } else {
                    format!("\nTrava: {}", gates.join(" e "))
                }
            );
        }
    }
    String::new()
}
