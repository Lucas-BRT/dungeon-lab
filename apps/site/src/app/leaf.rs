use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    metadata::{Metadata, Robots},
    router::{href, page},
    view::{View, component, view},
};

use super::walk::scene_backdrop;
use crate::app::t;

/// The dead end: one small room, one torch, a carrot on a plinth, the hand-drawn tapiti on the wall. With the scene it is a real leaf
/// room of the landing page's floor; without it, the same composition drawn in CSS and SVG. `missing` makes it the page for an address
/// that leads nowhere (the caller sets the 404 status).
#[component]
pub async fn leaf_chamber(cx: &Cx, missing: bool) -> Result<impl View> {
    Ok(view! {
        <link rel="stylesheet" href=(asset!("assets/leaf.css"))>
        scene_backdrop(mode: "leaf", class: "walk-backdrop leaf-backdrop")
        <section class="leaf">
            <div class="leaf-room" aria-hidden="true">
                <img
                    class="sketch"
                    src=(asset!("assets/brand/tapiti-real.svg"))
                    alt=""
                    width="277"
                    height="257"
                >
                <svg viewBox="0 0 220 120">
                    <path class="tunnel" d="M0 70H88" fill="none" stroke-width="14"></path>
                    <rect class="room" x="88" y="14" width="112" height="92" rx="4"></rect>
                    <rect class="plinth" x="128" y="78" width="32" height="20" rx="2"></rect>
                    <path
                        class="carrot"
                        d="M144 44c9 0 14 7 12 14l-9 17-9-17c-2-7 0-14 6-14z"
                    ></path>
                </svg>
            </div>
            <div class="leaf-text">
                if missing {
                    <h1>(t!(missing_title))</h1>
                    <p>(t!(missing_text))</p>
                    <p>
                        <a class="button primary" href=(href!(crate::app::_page::page))>
                            (t!(missing_back))
                        </a>
                    </p>
                } else {
                    <h1>(t!(chamber_title))</h1>
                    <p>(t!(chamber_text_one))</p>
                    <p>(t!(chamber_text_two))</p>
                    <p>
                        <a href=(href!(crate::app::_page::page))>(t!(missing_back))</a>
                    </p>
                }
            </div>
        </section>
    })
}

/// A leaf chamber: no page links here, and crawlers are told to skip it.
#[page(
    with(Metadata::new()
        .title(t!(chamber_title))
        .robots(Robots::new().index(false).follow(false)))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    Ok(view! { leaf_chamber(missing: false) })
}
