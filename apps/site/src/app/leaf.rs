use topcoat::{
    Result,
    context::Cx,
    metadata::{Metadata, Robots},
    router::page,
    view::{View, view},
};

use crate::app::t;

/// A leaf chamber: no page links here, and crawlers are told to skip it.
#[page(
    with(Metadata::new()
        .title(t!(chamber_title))
        .robots(Robots::new().index(false).follow(false)))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <section class="chamber-page">
            <svg viewBox="0 0 160 80" aria-hidden="true">
                <path class="tunnel" d="M0 40H92" fill="none" stroke-width="12"></path>
                <rect class="room" x="92" y="16" width="52" height="48" rx="3"></rect>
                <path
                    class="carrot"
                    d="M118 28c8 0 12 6 10 12l-8 14-8-14c-2-6 0-12 6-12z"
                ></path>
            </svg>
            <h1>(t!(chamber_title))</h1>
            <p>(t!(chamber_text_one))</p>
            <p>(t!(chamber_text_two))</p>
        </section>
    })
}
