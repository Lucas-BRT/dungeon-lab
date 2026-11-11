use topcoat::{
    Result,
    context::Cx,
    metadata::{Metadata, Robots},
    router::page,
    view::{View, view},
};

use crate::app::{leaf::leaf_chamber, t};

#[page(
    with(Metadata::new()
        .title(t!(missing_title))
        .robots(Robots::new().index(false).follow(false)))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        (topcoat::router::StatusCode::NOT_FOUND)
        leaf_chamber(missing: true)
    })
}
