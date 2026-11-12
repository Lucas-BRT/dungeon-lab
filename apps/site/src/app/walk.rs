//! The landing page as a walk down one floor: seven rooms, a depth gauge, a minimap and the floor itself as the backdrop.
//! Everything here is plain HTML and CSS; with scripts off, the page reads the same and its links still move you between rooms.

use std::sync::OnceLock;

use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    view::{Child, View, component, view},
};

use crate::{
    app::t,
    floor::{self, STATIONS},
};

/// Whether the page loads its 3D scene. `TAPITI_SCENE=off` turns it off for everyone without a deploy: the dimmed map of the same floor
/// stays behind the page. Read once, at the first request.
fn scene_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("TAPITI_SCENE").map_or(true, |value| value != "off"))
}

/// The id of each room's section, in walking order. The minimap and the gauge link to these.
pub const ROOM_IDS: [&str; STATIONS] = [
    "entrance", "shapes", "gate", "dig", "numbers", "run", "stairs",
];

/// The short name of room `i`, for the gauge and the minimap.
fn station_name(cx: &Cx, i: usize) -> String {
    match i {
        0 => t!(station_entrance),
        1 => t!(station_shapes),
        2 => t!(station_gate),
        3 => t!(station_dig),
        4 => t!(station_numbers),
        5 => t!(station_run),
        _ => t!(station_stairs),
    }
    .resolve(cx)
    .into_owned()
}

fn floor_label(cx: &Cx) -> String {
    t!(walk_floor_label, seed: floor::WALK_SEED, size: "medium")
        .resolve(cx)
        .into_owned()
}

/// One room of the walk: a section at least one screen tall, with its text on a plaque.
#[component]
pub async fn room(
    cx: &Cx,
    id: &'static str,
    station: u8,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="room" id=(id) data-station=(station.to_string())>
            <div class="plaque">(child)</div>
        </section>
    })
}

/// A floor figure on its map plate.
#[component]
pub async fn plate(cx: &Cx, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! { <figure class="plate">(child)</figure> })
}

/// The rail at the right edge: one tick per room, as links.
#[component]
pub async fn depth_gauge(cx: &Cx) -> Result<impl View> {
    let names: Vec<(usize, String)> = (0..STATIONS).map(|i| (i, station_name(cx, i))).collect();
    Ok(view! {
        <nav class="depth" aria-label=(t!(nav_sections))>
            <button
                class="depth-pause"
                type="button"
                hidden=(true)
                aria-pressed="false"
                data-pause=(t!(walk_pause).resolve(cx).into_owned())
                data-resume=(t!(walk_resume).resolve(cx).into_owned())
            ></button>
            <ol>
                for (i, name) in &names {
                    <li>
                        <a href=(format!("#{}", ROOM_IDS[*i]))>
                            <span class="depth-label">(name.as_str())</span>
                        </a>
                    </li>
                }
            </ol>
        </nav>
    })
}

/// The walk floor on a plate, with a link over each room where the page stops.
#[component]
pub async fn minimap(cx: &Cx) -> Result<impl View> {
    let svg = floor::walk_svg(floor_label(cx));
    let stations = floor::stations();
    let names: Vec<String> = (0..STATIONS).map(|i| station_name(cx, i)).collect();
    Ok(view! {
        <div class="minimap">
            <div class="minimap-floor" aria-hidden="true">(svg)</div>
            <span class="you" aria-hidden="true"></span>
            for (i, station) in stations.iter().enumerate() {
                <a
                    class="stop"
                    href=(format!("#{}", ROOM_IDS[i]))
                    aria-label=(names[i].as_str())
                    data-room=(station.room.to_string())
                    data-x=(format!("{:.1}", station.x))
                    data-y=(format!("{:.1}", station.y))
                    style=(format!(
                        "left:{:.2}%;top:{:.2}%",
                        station.left_pct,
                        station.top_pct,
                    ))
                ></a>
            }
        </div>
    })
}

/// The slot the 3D scene draws into: a fixed backdrop with a canvas and the data the script needs. `mode` is `landing` or `leaf`; what
/// the backdrop holds without the scene (the dimmed map, or nothing) is its child. Off with `TAPITI_SCENE=off`: then only the child shows.
#[component]
pub async fn scene_backdrop(
    cx: &Cx,
    mode: &'static str,
    class: &'static str,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let scene = scene_enabled();
    Ok(view! {
        <div
            class=(class)
            aria-hidden="true"
            if scene {
                data-mode=(mode)
                data-three=(asset!("assets/vendor/three-subset.min.js"))
                data-build=(asset!("assets/walk/build.js"))
                data-seed=(floor::WALK_SEED.to_string())
                data-size=(floor::WALK_SIZE_INDEX.to_string())
            }
        >
            (child)
            if scene {
                <canvas></canvas>
            }
        </div>
        if scene {
            <script type="module" src=(asset!("assets/walk/scene.js"))></script>
        }
    })
}

/// The floor behind the page. The scene draws over it; without that scene, it stays as a dimmed map.
#[component]
pub async fn walk_backdrop(cx: &Cx) -> Result<impl View> {
    let svg = floor::walk_svg(floor_label(cx));
    Ok(view! { scene_backdrop(mode: "landing", class: "walk-backdrop", (svg)) })
}

/// One step of "how a floor is dug": a room token, drawn in the shape it stands for, beside the step's text.
#[component]
pub async fn step(cx: &Cx, shape: &'static str, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <li class="step">
            <svg class="token" viewBox="0 0 48 48" aria-hidden="true">
                match shape {
                    "oct" => {
                        <polygon
                            class="c-oct"
                            points="15,4 33,4 44,15 44,33 33,44 15,44 4,33 4,15"
                        ></polygon>
                    }
                    "L" => {
                        <path class="c-L" d="M6 6h22v18h14v18H6z"></path>
                    }
                    "disc" => {
                        <circle class="c-disc" cx="24" cy="24" r="19"></circle>
                    }
                    _ => {
                        <rect class="c-rect" x="5" y="9" width="38" height="30" rx="1"></rect>
                    }
                }
            </svg>
            <div>(child)</div>
        </li>
    })
}
