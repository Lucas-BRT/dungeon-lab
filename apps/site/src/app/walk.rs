//! The landing page as a walk down one floor: seven rooms, a route (minimap and room list) and the floor itself as the backdrop.
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

/// The id of each room's section, in walking order. The route links to these.
pub const ROOM_IDS: [&str; STATIONS] = [
    "entrance", "shapes", "gate", "dig", "numbers", "run", "stairs",
];

/// The short name of room `i`, for the route.
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
    t!(walk_floor_label, seed: floor::WALK_SEED, level: floor::WALK_LEVEL)
        .resolve(cx)
        .into_owned()
}

/// One room of the walk: a section at least one screen tall on wide screens. Each room lays its text out its own way (see landing.css);
/// there is no box around it, the scene shows through.
#[component]
pub async fn room(
    cx: &Cx,
    id: &'static str,
    station: u8,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="room" id=(id) data-station=(station.to_string())>
            <div class="room-body">(child)</div>
        </section>
    })
}

/// A floor figure on its map plate, with a caption under it.
#[component]
pub async fn plate(cx: &Cx, caption: String, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <figure class="plate">
            (child)
            <figcaption>(caption)</figcaption>
        </figure>
    })
}

/// The route at the right edge: the floor as a small map with a "you are here" dot, then every room by name, as links, the current one
/// marked. All of it works without scripts; the script only moves the dot and the mark. The pause control appears with the scene.
#[component]
pub async fn route(cx: &Cx) -> Result<impl View> {
    let svg = floor::walk_svg(floor_label(cx));
    let stations = floor::stations();
    let names: Vec<String> = (0..STATIONS).map(|i| station_name(cx, i)).collect();
    Ok(view! {
        <nav class="route" aria-label=(t!(nav_sections))>
            <div class="minimap" aria-hidden="true">
                <div class="minimap-floor">(svg)</div>
                <span class="you"></span>
                for (i, station) in stations.iter().enumerate() {
                    <a
                        class="stop"
                        tabindex="-1"
                        href=(format!("#{}", ROOM_IDS[i]))
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
            <ol class="route-list">
                for (i, name) in names.iter().enumerate() {
                    <li><a href=(format!("#{}", ROOM_IDS[i]))>(name.as_str())</a></li>
                }
            </ol>
            <button
                class="route-pause"
                type="button"
                hidden=(true)
                aria-pressed="false"
                data-pause=(t!(walk_pause).resolve(cx).into_owned())
                data-resume=(t!(walk_resume).resolve(cx).into_owned())
            ></button>
        </nav>
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
                data-level=(floor::WALK_LEVEL.to_string())
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

/// A room drawn in one of the five shapes, coloured like the floors: `rect`, `disc`, `oct`, `L` or `star`.
#[component]
pub async fn token(cx: &Cx, shape: &'static str) -> Result<impl View> {
    Ok(view! {
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
                "star" => {
                    <polygon
                        class="c-star"
                        points="26.6,4.2 21.4,4.2 20.0,14.3 11.8,8.1 8.1,11.8 14.3,20.0 4.2,21.4 4.2,26.6 14.3,28.0 8.1,36.2 11.8,39.9 20.0,33.7 21.4,43.8 26.6,43.8 28.0,33.7 36.2,39.9 39.9,36.2 33.7,28.0 43.8,26.6 43.8,21.4 33.7,20.0 39.9,11.8 36.2,8.1 28.0,14.3"
                    ></polygon>
                }
                _ => {
                    <rect class="c-rect" x="5" y="9" width="38" height="30" rx="1"></rect>
                }
            }
        </svg>
    })
}

/// One step of "how a floor is dug": a room token beside the step's text.
#[component]
pub async fn step(cx: &Cx, shape: &'static str, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <li class="step">
            token(shape: shape)
            <div>(child)</div>
        </li>
    })
}
