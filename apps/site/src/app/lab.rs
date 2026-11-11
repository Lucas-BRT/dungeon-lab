//! The browser lab. Topcoat renders the whole page, including the parameter form (from `tapiti-schema`, so it never drifts from the
//! desktop app and the docs). `assets/lab/lab.js` only binds to that markup and drives the WebAssembly generator, the worker pool and
//! the canvas. The whole site is in English.

use tapiti_schema::{FieldKind, ParamField, ParamGroup};
use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    l10n::{fluent::messages, locale},
    metadata::Metadata,
    router::page,
    view::{View, component, view},
};

// `lab/l10n/en.ftl`: this module's own messages, kept next to the page.
messages!("{mod}/l10n/{locale}.ftl");

/// A message of this module.
macro_rules! e {
    ($($t:tt)*) => {
        t!($($t)*).resolve_in(&locale!("en"))
    };
}

/// Messages the page script needs while it runs, as `id -> text` with `{name}` where a value goes. Sent to the script in one attribute.
fn dictionary() -> String {
    macro_rules! dict {
        ($($id:ident $(($($arg:ident),*))?),* $(,)?) => {{
            let mut map = serde_json::Map::new();
            $(map.insert(
                stringify!($id).to_string(),
                serde_json::Value::String(
                    e!($id $(, $($arg: concat!("{", stringify!($arg), "}")),*)?).into_owned(),
                ),
            );)*
            map
        }};
    }
    let map = dict! {
        lab_map_instructions, lab_3d_instructions,
        lab_level_1, lab_level_2, lab_level_3, lab_level_4, lab_level_5,
        lab_seed_error, lab_copied, lab_no_clipboard,
        lab_changed_none, lab_changed_some(n),
        lab_exp_shapes_done, lab_exp_cycles_done, lab_exp_junctions_done, lab_exp_rooms_done,
        lab_junction_inf,
        lab_perf_40_result(ms, invalid), lab_perf_800_run,
        lab_perf_800_result(one, workers, many, invalid), lab_perf_800_fail(error),
        lab_err_int(min, max), lab_err_range(min, max),
        lab_help_prefix(text), lab_gen_failed, lab_valid, lab_valid_detail(draws, ms), lab_invalid(n), lab_invalid_detail(draws),
        lab_panel_title, lab_panel_eyebrow(level, seed),
        lab_g_size, lab_g_cycles, lab_g_paths,
        lab_m_rooms, lab_m_rooms_help, lab_m_leaf_chambers, lab_m_leaf_chambers_help,
        lab_m_junctions, lab_m_junctions_help, lab_m_edges, lab_m_edges_help,
        lab_m_beta, lab_m_beta_help, lab_m_beta_target, lab_m_beta_target_help,
        lab_m_beta_max, lab_m_beta_max_help, lab_m_delta, lab_m_delta_help,
        lab_m_mean_degree, lab_m_mean_degree_help, lab_m_leaves, lab_m_leaves_help,
        lab_m_bridges, lab_m_bridges_help, lab_m_articulations, lab_m_articulations_help,
        lab_m_hops, lab_m_hops_help, lab_m_diameter, lab_m_diameter_help,
        lab_m_routes, lab_m_routes_help, lab_m_girth, lab_m_girth_help,
        lab_m_side_branches, lab_m_side_branches_help, lab_m_branch_depth, lab_m_branch_depth_help,
        lab_m_max_degree, lab_m_max_degree_help, lab_m_geo_path, lab_m_geo_path_help,
        lab_m_lambda2, lab_m_lambda2_help,
        lab_shapes_title, lab_log_title, lab_legend_title, lab_legend_shapes, lab_legend_roles,
        lab_class_rect, lab_class_disc, lab_class_oct, lab_class_L, lab_class_star,
        lab_role_start, lab_role_stairs, lab_role_ambush, lab_role_boss, lab_role_rest,
        lab_role_key, lab_role_switch, lab_role_trap, lab_role_prize, lab_role_empty, lab_role_plain,
        lab_leaf_role(role),
        lab_node_junction, lab_node_room(id), lab_node_point,
        lab_bits_keys(n), lab_bits_switches(n),
        lab_tip_room(role, id, class, area, used, possible), lab_tip_items(list),
        lab_tip_junction(degree, x, y), lab_tip_corridor(id, from, to, n, length),
        lab_tip_locked(list), lab_tip_and,
        lab_gate_key(bits), lab_gate_keys(bits), lab_gate_switch(bits), lab_gate_switches(bits),
        lab_v_v1(a), lab_v_v2_rooms(a, b), lab_v_v4_reach(a, b), lab_v_v4_short(a, b, c),
        lab_v_v2_straight(a, b), lab_v_v2_past_door(a, b), lab_v_v2_corr_room(a, b),
        lab_v_v2_corr_corr(a, b), lab_v_v3_diag(a), lab_v_v3_back(a), lab_v_v3_circle(a),
        lab_v_v3_doors(a), lab_v_v4_alphabet(a), lab_v_v4_turn(a, b), lab_v_v4_segment(a, b),
        lab_v_v4_angle(a), lab_v_v5_doors(a, b), lab_v_v5_hops(a, b), lab_v_v5_distinct,
        lab_v_t1(a, b), lab_v_t2(a, b), lab_v_t3(a, b), lab_v_t4(a, b), lab_v_t5(a, b),
        lab_v_p1, lab_v_p2, lab_v_p3,
        lab_l_stuck_candidates(a), lab_l_stuck_near(a), lab_l_left_out(a), lab_l_stairs(a, b),
        lab_l_beta(a, b), lab_l_block(a, b), lab_l_leaves(a, b), lab_l_locks(a, b), lab_l_lock_bits,
        lab_l_routes(a, b), lab_l_hub(a), lab_l_cycle(a), lab_l_branches(a, b), lab_l_jitter(a, b),
        lab_l_caps,
    };
    serde_json::Value::Object(map).to_string()
}

/// `1234.5` as `1,234.5`, `10000` as `10,000`: the way the page script writes numbers.
fn number(value: f64) -> String {
    let text = if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        let text = format!("{value:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let (sign, digits) = text
        .strip_prefix('-')
        .map_or(("", text.as_str()), |d| ("-", d));
    let (whole, rest) = digits.split_once('.').map_or((digits, ""), |(w, r)| (w, r));
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if rest.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{rest}")
    }
}

/// The field's own explanation plus what it accepts.
fn field_help(field: &ParamField) -> String {
    let allowed = match &field.kind {
        FieldKind::Boolean => e!(lab_field_bool).into_owned(),
        FieldKind::Choice(options) => {
            let list = options
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            e!(lab_field_options, list: list).into_owned()
        }
        FieldKind::Number { min, max, integer } => {
            let (min, max) = (number(*min), number(*max));
            if *integer {
                e!(lab_field_range_int, min: min, max: max).into_owned()
            } else {
                e!(lab_field_range, min: min, max: max).into_owned()
            }
        }
    };
    format!("{} {allowed}", field.help.as_str())
}

#[page(
    with(Metadata::new()
        .title("Lab")
        .description(
            "Change a parameter and Tapiti digs the floor again, in your browser, with WebAssembly. Every rule of the model is a field you can edit.",
        ))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let groups = tapiti_schema::groups();
    let levels_help = e!(lab_levels_help);
    let seed_help = e!(lab_seed_help);
    let overlays_help = e!(lab_overlays_help);
    let params_help = e!(lab_params_help);
    let overlays = [
        (
            "cells",
            e!(lab_overlay_cells),
            e!(lab_overlay_cells_help),
            true,
        ),
        (
            "labels",
            e!(lab_overlay_labels),
            e!(lab_overlay_labels_help),
            true,
        ),
        (
            "ports",
            e!(lab_overlay_ports),
            e!(lab_overlay_ports_help),
            false,
        ),
        (
            "pairs",
            e!(lab_overlay_pairs),
            e!(lab_overlay_pairs_help),
            false,
        ),
        (
            "candidates",
            e!(lab_overlay_candidates),
            e!(lab_overlay_candidates_help),
            false,
        ),
    ];
    let experiments = [
        ("shapes", e!(lab_exp_shapes), e!(lab_exp_shapes_detail)),
        ("cycles", e!(lab_exp_cycles), e!(lab_exp_cycles_detail)),
        (
            "junctions",
            e!(lab_exp_junctions),
            e!(lab_exp_junctions_detail),
        ),
        ("rooms", e!(lab_exp_rooms), e!(lab_exp_rooms_detail)),
    ];
    Ok(view! {
        <link rel="stylesheet" href=(asset!("assets/lab/lab.css"))>
        <div
            id="lab"
            class="lab"
            lang="en"
            data-tab="controls"
            data-i18n=(dictionary())
            data-worker=(asset!("assets/lab/worker.js"))
            data-three=(asset!("assets/vendor/three-subset.min.js"))
            data-build=(asset!("assets/walk/build.js"))
            data-scene=(asset!("assets/walk/scene.js"))
        >
            <noscript><p class="lab-noscript">(e!(lab_needs_js))</p></noscript>
            <div class="lab-tabs" role="tablist" aria-label=(e!(lab_tabs_aria))>
                <button
                    type="button"
                    role="tab"
                    id="tab-controls"
                    aria-selected="true"
                    aria-controls="controls"
                    data-tab="controls"
                >
                    (e!(lab_tab_controls))
                </button>
                <button
                    type="button"
                    role="tab"
                    id="tab-reading"
                    aria-selected="false"
                    aria-controls="results"
                    data-tab="reading"
                >
                    (e!(lab_tab_reading))
                </button>
            </div>
            <section id="controls" class="lab-panel" aria-label=(e!(lab_controls_aria))>
                <p class="eyebrow">(e!(lab_eyebrow))</p>
                <h1>"Tapiti"</h1>
                <p class="intro-copy">(e!(lab_intro))</p>
                <div class="level-section">
                    <div class="section-title">
                        <h2>(e!(lab_levels_title))</h2>
                        help_button(text: &levels_help)
                    </div>
                    <div class="level-list">
                        for n in 1..=5 {
                            <button
                                type="button"
                                class="level-button"
                                data-level=(n.to_string())
                                aria-pressed="false"
                                aria-label=(e!(lab_level_aria, n: n))
                            >
                                (e!(lab_level_button, n: n))
                            </button>
                        }
                    </div>
                    <p class="level-description" id="level-description"></p>
                </div>
                <div class="seed-card">
                    <label for="seed-input">
                        (e!(lab_seed))
                        help_button(text: &seed_help)
                    </label>
                    <div class="seed-control">
                        <button
                            type="button"
                            class="icon-button"
                            data-seed-step="-1"
                            aria-label=(e!(lab_seed_prev))
                        >
                            "−"
                        </button>
                        <button
                            type="button"
                            class="icon-button"
                            data-seed-step="1"
                            aria-label=(e!(lab_seed_next))
                        >
                            "+"
                        </button>
                        <input
                            id="seed-input"
                            type="number"
                            min="0"
                            max="4294967295"
                            step="1"
                            value="1"
                            aria-label=(e!(lab_seed_aria))
                        >
                        <button type="button" class="secondary-button" id="seed-random">
                            (e!(lab_seed_random))
                        </button>
                    </div>
                </div>
                <div class="action-row">
                    <button type="button" class="secondary-button" id="copy-params">
                        (e!(lab_copy))
                    </button>
                    <button type="button" class="secondary-button" id="reset-level">
                        (e!(lab_reset_level))
                    </button>
                </div>
                <p class="changed-status" id="changed-status"></p>
                <details class="howto">
                    <summary>(e!(lab_howto_title))</summary>
                    <p>(e!(lab_howto_text))</p>
                </details>
                <div class="section-title">
                    <h2>(e!(lab_overlays_title))</h2>
                    help_button(text: &overlays_help)
                </div>
                <div class="overlay-list">
                    for (key, name, text, on) in &overlays {
                        <label class="overlay-option">
                            <input type="checkbox" data-overlay=(*key) checked=(*on)>
                            <span>(name.as_ref())</span>
                            help_button(text: text.as_ref())
                        </label>
                    }
                </div>
                <details class="tool-group">
                    <summary>(e!(lab_experiments_title))</summary>
                    <p class="group-intro">(e!(lab_experiments_intro))</p>
                    <div class="experiment-buttons">
                        for (kind, name, detail) in &experiments {
                            <button
                                type="button"
                                class="experiment-button"
                                data-experiment=(*kind)
                            >
                                <strong>(name.as_ref())</strong>
                                <span>(detail.as_ref())</span>
                            </button>
                        }
                    </div>
                </details>
                <div class="section-title">
                    <h2>(e!(lab_params_title))</h2>
                    help_button(text: &params_help)
                </div>
                <div class="parameter-groups" id="parameter-groups">
                    for group in &groups {
                        param_group(group: group)
                    }
                    <details class="parameter-group">
                        <summary>
                            <span>(e!(lab_junction_title))</span>
                            <span class="group-count">(e!(lab_junction_badge))</span>
                        </summary>
                        <p class="group-intro">(e!(lab_junction_text))</p>
                        <div class="readonly-setting">
                            <span>(e!(lab_junction_current))</span>
                            <strong id="junction-value"></strong>
                        </div>
                    </details>
                </div>
                <details class="tool-group">
                    <summary>(e!(lab_perf_title))</summary>
                    <p class="group-intro">(e!(lab_perf_intro))</p>
                    <div class="tool-buttons">
                        <button type="button" class="secondary-button" id="bench-40">
                            (e!(lab_perf_40))
                        </button>
                        <button type="button" class="secondary-button" id="bench-800">
                            (e!(lab_perf_800))
                        </button>
                    </div>
                    <p class="status-line" id="bench-status" aria-live="polite">
                        (e!(lab_perf_idle))
                    </p>
                </details>
            </section>
            <section id="view" aria-label=(e!(lab_map_view))>
                <canvas
                    id="floor-map"
                    tabindex="0"
                    role="img"
                    aria-label=(e!(lab_map_label))
                    aria-describedby="map-instructions"
                ></canvas>
                <canvas
                    id="floor-3d"
                    tabindex="0"
                    role="img"
                    aria-label=(e!(lab_3d_label))
                    hidden=(true)
                ></canvas>
                <div class="map-tooltip" id="map-tooltip" role="tooltip" hidden=(true)></div>
                <div
                    class="map-toolbar"
                    role="toolbar"
                    aria-label=(e!(lab_toolbar_aria))
                >
                    <div
                        class="view-switch"
                        role="group"
                        aria-label=(e!(lab_view_aria))
                        hidden=(true)
                    >
                        <button
                            type="button"
                            class="map-tool-button"
                            id="view-plan"
                            aria-pressed="true"
                        >
                            (e!(lab_view_plan))
                        </button>
                        <button
                            type="button"
                            class="map-tool-button"
                            id="view-3d"
                            aria-pressed="false"
                        >
                            (e!(lab_view_3d))
                        </button>
                    </div>
                    <button
                        type="button"
                        class="map-tool-button"
                        id="zoom-out"
                        aria-label=(e!(lab_zoom_out))
                        title=(e!(lab_zoom_out))
                    >
                        "−"
                    </button>
                    <span class="zoom-label" id="zoom-label" aria-live="polite">
                        "100%"
                    </span>
                    <button
                        type="button"
                        class="map-tool-button"
                        id="zoom-in"
                        aria-label=(e!(lab_zoom_in))
                        title=(e!(lab_zoom_in))
                    >
                        "+"
                    </button>
                    <button
                        type="button"
                        class="map-tool-button fit-button"
                        id="zoom-fit"
                    >
                        (e!(lab_fit))
                    </button>
                    <button
                        type="button"
                        class="map-tool-button reading-toggle"
                        id="reading-toggle"
                        aria-expanded="false"
                        aria-controls="results"
                    >
                        (e!(lab_reading))
                    </button>
                </div>
                <p class="map-instructions" id="map-instructions">
                    (e!(lab_map_instructions))
                </p>
            </section>
            <section id="results" class="lab-panel" aria-label=(e!(lab_results_aria))>
                <p class="status-line">(e!(lab_loading))</p>
            </section>
            <div
                class="toast"
                id="lab-toast"
                role="status"
                aria-live="polite"
                hidden=(true)
            ></div>
            <div class="help-tooltip" id="help-tooltip" role="tooltip" hidden=(true)></div>
        </div>
        <script type="module" src=(asset!("assets/lab/lab.js"))></script>
    })
}

/// The small "?" that shows a text on hover, focus or click. The page script handles it for every button with `data-help`.
#[component]
async fn help_button(cx: &Cx, text: &str) -> Result<impl View> {
    Ok(view! {
        <button
            type="button"
            class="help-button"
            data-help=(text)
            aria-label=(e!(lab_help_prefix, text: text))
            aria-expanded="false"
            aria-describedby="help-tooltip"
        >
            "?"
        </button>
    })
}

/// One group of the parameter form.
#[component]
async fn param_group(cx: &Cx, group: &ParamGroup) -> Result<impl View> {
    Ok(view! {
        <details
            class="parameter-group"
            data-group=(group.id.as_str())
            open=(group.open_by_default)
        >
            <summary>
                <span>(group.title.as_str())</span>
                <span class="group-count">(group.fields.len().to_string())</span>
            </summary>
            <p class="group-intro">(group.intro.as_str())</p>
            <button type="button" class="text-button group-reset">
                (e!(lab_group_reset))
            </button>
            for field in &group.fields {
                param_field(field: field)
            }
        </details>
    })
}

/// One parameter: its label, help, and an input the page script fills with the level's value.
#[component]
async fn param_field(cx: &Cx, field: &ParamField) -> Result<impl View> {
    let id = format!("param-{}", field.path.replace('.', "-"));
    let help = field_help(field);
    Ok(view! {
        <div class="field">
            <div class="field-title">
                <label for=(id.as_str())>(field.label.as_str())</label>
                help_button(text: &help)
            </div>
            match &field.kind {
                FieldKind::Boolean => {
                    <input
                        id=(id.as_str())
                        name=(field.path.as_str())
                        type="checkbox"
                        data-kind="boolean"
                    >
                }
                FieldKind::Choice(options) => {
                    <select
                        id=(id.as_str())
                        name=(field.path.as_str())
                        data-kind="choice"
                    >
                        for (value, name) in options {
                            <option value=(value.as_str())>(name.as_str())</option>
                        }
                    </select>
                }
                FieldKind::Number { min, max, integer } => {
                    <input
                        id=(id.as_str())
                        name=(field.path.as_str())
                        type="number"
                        data-kind="number"
                        data-integer=(*integer)
                        min=(min.to_string())
                        max=(max.to_string())
                        step=(if *integer { "1" } else { "any" })
                        inputmode="decimal"
                    >
                }
            }
            <span class="field-error" aria-live="polite"></span>
        </div>
    })
}
