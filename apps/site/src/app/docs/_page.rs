use topcoat::{
    Result,
    context::Cx,
    router::{error::redirect, href, page},
    view::View,
};

use super::slug;

/// `/docs` has no page of its own: it opens the first one.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    Err::<(), _>(redirect(href!(slug::_page::page, slug: "getting-started").resolve(cx)).into())
}
