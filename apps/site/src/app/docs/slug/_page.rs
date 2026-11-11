use topcoat::{
    Result,
    context::Cx,
    metadata::Metadata,
    router::{error::not_found, page},
    view::{View, view},
};

use super::slug;
use crate::{app::t, docs::docs, floor::TrustedHtml};

#[page(
    with(Metadata::from_fn(
        async |cx| {
            let title = docs().page(slug(cx)).map(|found| found.title.clone());
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
    let Some(page) = docs().page(slug(cx)) else {
        return Err(not_found().into());
    };
    let body = TrustedHtml(page.html.clone());
    Ok(view! {
        super::super::frame(
            current: page.slug,
            <div class="docs-content">
                <article class="docs-article">
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
