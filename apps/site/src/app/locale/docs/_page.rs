use topcoat::{
    Result,
    context::Cx,
    l10n::locale,
    router::{error::redirect, href, page},
    view::View,
};

use super::slug;
use crate::app::locale::path_locale;

/// `/{locale}/docs` has no page of its own: it opens the first one.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let here = locale(cx).clone();
    Err::<(), _>(
        redirect(href!(slug::_page::page, path_locale: &here, slug: "getting-started").resolve(cx))
            .into(),
    )
}
