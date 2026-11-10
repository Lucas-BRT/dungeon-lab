use topcoat::{
    Result,
    context::Cx,
    l10n::locale,
    metadata::Metadata,
    router::{error::not_found, page},
    view::{View, view},
};

use super::slug;
use crate::{app::t, docs::docs, floor::TrustedHtml};

#[page(
    with(Metadata::from_fn(
        async |cx| {
            let here = locale(cx).id().to_string();
            let title = docs().page(&here, slug(cx)).map(|found| found.title.clone());
            Ok(
                match title {
                    Some(title) => Metadata::new().title(format!("{title} · Tapiti")),
                    None => Metadata::new(),
                },
            )
        },
    ))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let here = locale(cx).clone();
    let id = here.id().to_string();
    let Some(page) = docs().page(&id, slug(cx)) else {
        return Err(not_found().into());
    };
    let body = TrustedHtml(page.html.clone());
    // An untranslated page shows the English text, and says so to screen readers too.
    let article_lang = if page.fallback { "en".to_string() } else { id };
    Ok(view! {
        super::super::frame(
            current: page.slug,
            <div class="docs-content">
                <article class="docs-article" lang=(article_lang)>
                    if page.fallback {
                        <p class="docs-banner" lang=(here.id())>(t!(docs_fallback))</p>
                    }
                    (body)
                    <p class="docs-edit">
                        <a href=(page.source_url.as_str())>(t!(docs_edit))</a>
                    </p>
                </article>
                <nav class="docs-toc" aria-label=(t!(docs_toc_label))>
                    <ul>
                        for entry in &page.toc {
                            <li class=(if entry.level > 2 { "sub" } else { "" })>
                                <a href=(format!("#{}", entry.id))>
                                    (entry.text.as_str())
                                </a>
                            </li>
                        }
                    </ul>
                </nav>
            </div>
        )
    })
}
