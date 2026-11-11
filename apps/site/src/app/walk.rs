//! The landing page as a walk down one floor: seven rooms, a depth gauge, a minimap and the floor itself as the backdrop.
//! Everything here is plain HTML and CSS; with scripts off, the page reads the same and its links still move you between rooms.

use topcoat::{
    Result,
    context::Cx,
    view::{Child, View, component, view},
};

use crate::{
    app::t,
    floor::{self, STATIONS},
};

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
    t!(walk_floor_label, seed: floor::WALK_SEED, level: floor::WALK_LEVEL)
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

/// The floor behind the page. The scene of a later phase draws over it; without that scene, it stays as a dimmed map.
#[component]
pub async fn walk_backdrop(cx: &Cx) -> Result<impl View> {
    let svg = floor::walk_svg(floor_label(cx));
    Ok(view! { <div class="walk-backdrop" aria-hidden="true">(svg)</div> })
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
