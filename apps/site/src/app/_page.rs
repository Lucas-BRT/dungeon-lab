use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    router::{href, page},
    view::{View, view},
};

use super::walk::{ROOM_IDS, plate, room, route, step, token, walk_backdrop};
use crate::{
    app::{leaf, t},
    floor::{self, TrustedHtml},
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let shapes = floor::shapes(t!(shapes_figure_label).resolve(cx).into_owned());
    let (rejected, violations) =
        floor::rejected(t!(rejected_figure_label).resolve(cx).into_owned());
    let kept_caption = t!(validator_kept_caption, seed: floor::SHAPES_SEED)
        .resolve(cx)
        .into_owned();
    let rejected_caption = t!(validator_caption, seed: floor::REJECTED_SEED)
        .resolve(cx)
        .into_owned();
    let speed = floor::speed();
    // For whoever reads the source.
    let hint = TrustedHtml(format!(
        "<!-- a dead end, off the map: {} -->",
        href!(leaf::page).resolve(cx)
    ));
    Ok(view! {
        <link rel="stylesheet" href=(asset!("assets/landing.css"))>
        <div class="walk">
            walk_backdrop()
            route()
            <div class="rooms">
                room(
                    id: ROOM_IDS[0],
                    station: 0,
                    <h1>(t!(hero_title))</h1>
                    <p class="lede">(t!(hero_text))</p>
                    <div class="cta-row">
                        <a class="button primary" href=(href!(crate::app::lab::page))>
                            (t!(cta_lab))
                        </a>
                        <a class="button" href=(href!(crate::app::docs::_page::page))>
                            (t!(cta_docs))
                        </a>
                    </div>
                    <p class="caption">
                        (t!(walk_caption, seed: floor::WALK_SEED, size: "medium"))
                    </p>
                )
                room(
                    id: ROOM_IDS[1],
                    station: 1,
                    <h2>(t!(shapes_title))</h2>
                    <p>(t!(shapes_text))</p>
                    <ul class="shape-row">
                        <li>
                            token(shape: "rect")
                            (t!(shape_rect))
                        </li>
                        <li>
                            token(shape: "disc")
                            (t!(shape_disc))
                        </li>
                        <li>
                            token(shape: "oct")
                            (t!(shape_oct))
                        </li>
                        <li>
                            token(shape: "L")
                            (t!(shape_l))
                        </li>
                        <li>
                            token(shape: "star")
                            (t!(shape_star))
                        </li>
                    </ul>
                )
                room(
                    id: ROOM_IDS[2],
                    station: 2,
                    <div class="table-text">
                        <h2>(t!(validator_title))</h2>
                        <p>(t!(validator_text))</p>
                    </div>
                    <div class="table-plates">
                        plate(caption: kept_caption, (shapes))
                        <div class="rejected">
                            plate(caption: rejected_caption, (rejected))
                            <ul class="verdict">
                                for violation in violations {
                                    <li><code>(violation)</code></li>
                                }
                            </ul>
                        </div>
                    </div>
                )
                room(
                    id: ROOM_IDS[3],
                    station: 3,
                    <h2>(t!(descent_title))</h2>
                    <ol class="steps">
                        step(
                            shape: "rect",
                            <h3>(t!(descent_seed_title))</h3>
                            <p>(t!(descent_seed_text))</p>
                        )
                        step(
                            shape: "oct",
                            <h3>(t!(descent_rooms_title))</h3>
                            <p>(t!(descent_rooms_text))</p>
                        )
                        step(
                            shape: "L",
                            <h3>(t!(descent_tunnels_title))</h3>
                            <p>(t!(descent_tunnels_text))</p>
                        )
                        step(
                            shape: "disc",
                            <h3>(t!(descent_check_title))</h3>
                            <p>(t!(descent_check_text))</p>
                        )
                    </ol>
                )
                room(
                    id: ROOM_IDS[4],
                    station: 4,
                    <h2>(t!(proof_label))</h2>
                    <dl class="register">
                        <div>
                            <dt>(t!(proof_speed_label))</dt>
                            <dd>(t!(proof_speed_value, ms: speed.median_ms))</dd>
                        </div>
                        <div>
                            <dt>(t!(proof_valid_label, total: speed.total))</dt>
                            <dd>
                                (t!(proof_valid_value, valid: speed.valid, total: speed.total))
                            </dd>
                        </div>
                        <div>
                            <dt>(t!(proof_seeds_label))</dt>
                            <dd>(t!(proof_seeds_value))</dd>
                        </div>
                    </dl>
                )
                room(
                    id: ROOM_IDS[5],
                    station: 5,
                    <h2>(t!(run_title))</h2>
                    <dl class="run-panel">
                        <div>
                            <dt>(t!(run_browser))</dt>
                            <dd>
                                <a href=(href!(crate::app::lab::page))>
                                    (t!(run_browser_link))
                                </a>
                            </dd>
                        </div>
                        <div>
                            <dt>(t!(run_desktop))</dt>
                            <dd><code>"cargo run --release"</code></dd>
                        </div>
                        <div>
                            <dt>(t!(run_crate))</dt>
                            <dd>
                                <code>
                                    "tapiti-core = { git = \"https://github.com/Lucas-BRT/tapiti\" }"
                                </code>
                            </dd>
                        </div>
                        <div>
                            <dt>"Docker"</dt>
                            <dd>
                                <code>
                                    "docker compose -f compose.local.yml up --build"
                                </code>
                            </dd>
                        </div>
                    </dl>
                )
                room(
                    id: ROOM_IDS[6],
                    station: 6,
                    <h2>(t!(final_title))</h2>
                    <p class="lede">(t!(final_text))</p>
                    <div class="cta-row">
                        <a class="button primary" href=(href!(crate::app::lab::page))>
                            (t!(cta_lab))
                        </a>
                        <a class="button" href=(href!(crate::app::docs::_page::page))>
                            (t!(cta_docs))
                        </a>
                    </div>
                )
            </div>
            (hint)
        </div>
    })
}
