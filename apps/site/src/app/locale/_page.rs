use topcoat::{
    Result,
    context::Cx,
    l10n::locale,
    router::{href, page},
    view::{Child, View, component, view},
};

use crate::{
    app::{
        locale::{leaf, path_locale},
        t,
    },
    floor::{self, TrustedHtml},
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let hero = floor::hero(t!(hero_floor_label).resolve(cx).into_owned());
    let shapes = floor::shapes(t!(shapes_figure_label).resolve(cx).into_owned());
    let (rejected, violations) =
        floor::rejected(t!(rejected_figure_label).resolve(cx).into_owned());
    let speed = floor::speed();
    // For whoever reads the source.
    let hint = TrustedHtml(format!(
        "<!-- a dead end, off the map: {} -->",
        href!(leaf::page, path_locale: locale(cx)).resolve(cx)
    ));
    Ok(view! {
        <section class="hero">
            <div class="hero-copy">
                <h1>(t!(hero_title))</h1>
                <p>(t!(hero_text))</p>
                <div class="cta-row">
                    <a class="button primary" href=(href!(crate::app::lab))>
                        (t!(cta_lab))
                    </a>
                    <a
                        class="button"
                        href=(href!(
                            crate::app::locale::docs::_page::page,
                            path_locale: locale(cx),
                        ))
                    >
                        (t!(cta_docs))
                    </a>
                </div>
            </div>
            <figure class="hero-floor">(hero)</figure>
        </section>

        <section class="proof" aria-label=(t!(proof_label))>
            <div class="stat">
                <p class="stat-value">(t!(proof_speed_value, ms: speed.median_ms))</p>
                <p class="stat-label">(t!(proof_speed_label))</p>
            </div>
            <div class="stat">
                <p class="stat-value">
                    (t!(proof_valid_value, valid: speed.valid, total: speed.total))
                </p>
                <p class="stat-label">(t!(proof_valid_label, total: speed.total))</p>
            </div>
            <div class="stat">
                <p class="stat-value">(t!(proof_seeds_value))</p>
                <p class="stat-label">(t!(proof_seeds_label))</p>
            </div>
        </section>

        <section class="feature">
            <div class="feature-copy">
                <h2>(t!(shapes_title))</h2>
                <p>(t!(shapes_text))</p>
                <ul class="shape-legend">
                    <li>
                        <i class="swatch c-rect"></i>
                        (t!(shape_rect))
                    </li>
                    <li>
                        <i class="swatch c-disc"></i>
                        (t!(shape_disc))
                    </li>
                    <li>
                        <i class="swatch c-oct"></i>
                        (t!(shape_oct))
                    </li>
                    <li>
                        <i class="swatch c-L"></i>
                        (t!(shape_l))
                    </li>
                    <li>
                        <i class="swatch c-star"></i>
                        (t!(shape_star))
                    </li>
                </ul>
            </div>
            <figure class="feature-figure">(shapes)</figure>
        </section>

        <section class="feature reverse">
            <div class="feature-copy">
                <h2>(t!(validator_title))</h2>
                <p>(t!(validator_text))</p>
                <div class="verdict">
                    <p class="verdict-caption">
                        (t!(validator_caption, seed: floor::REJECTED_SEED))
                    </p>
                    for violation in violations {
                        <code>(violation)</code>
                    }
                </div>
            </div>
            <figure class="feature-figure">(rejected)</figure>
        </section>

        <section class="descent" aria-labelledby="descent-title">
            <h2 id="descent-title">(t!(descent_title))</h2>
            <ol>
                chamber(
                    shape: "rect",
                    <h3>(t!(descent_seed_title))</h3>
                    <p>(t!(descent_seed_text))</p>
                )
                chamber(
                    shape: "oct",
                    <h3>(t!(descent_rooms_title))</h3>
                    <p>(t!(descent_rooms_text))</p>
                )
                chamber(
                    shape: "L",
                    <h3>(t!(descent_tunnels_title))</h3>
                    <p>(t!(descent_tunnels_text))</p>
                )
                chamber(
                    shape: "disc",
                    <h3>(t!(descent_check_title))</h3>
                    <p>(t!(descent_check_text))</p>
                )
            </ol>
        </section>

        <section class="run" aria-labelledby="run-title">
            <h2 id="run-title">(t!(run_title))</h2>
            <dl class="run-panel">
                <div>
                    <dt>(t!(run_desktop))</dt>
                    <dd><code>"cargo run --release"</code></dd>
                </div>
                <div>
                    <dt>(t!(run_browser))</dt>
                    <dd><a href=(href!(crate::app::lab))>(t!(run_browser_link))</a></dd>
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
                        <code>"docker compose -f compose.local.yml up --build"</code>
                    </dd>
                </div>
            </dl>
        </section>

        (hint)

        <section class="final-cta">
            <h2>(t!(final_title))</h2>
            <div class="cta-row">
                <a class="button primary" href=(href!(crate::app::lab))>
                    (t!(cta_lab))
                </a>
                <a
                    class="button"
                    href=(href!(
                        crate::app::locale::docs::_page::page,
                        path_locale: locale(cx),
                    ))
                >
                    (t!(cta_docs))
                </a>
            </div>
        </section>
    })
}

/// One chamber of the burrow: a room token, drawn in the shape it stands for, beside the step's text.
#[component]
async fn chamber(cx: &Cx, shape: &'static str, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <li class="chamber">
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
