use topcoat::{
    Result,
    context::Cx,
    metadata::{Metadata, Robots},
    router::{StatusCode, page},
    view::{View, view},
};

use super::slug;
use crate::{
    app::{leaf::leaf_chamber, t},
    docs::docs,
    floor::TrustedHtml,
};

#[page(
    with(Metadata::from_fn(
        async |cx| {
            let title = docs().page(slug(cx)).map(|found| found.title.clone());
            Ok(
                match title {
                    Some(title) => Metadata::new().title(format!("{title} · Tapiti")),
                    None => Metadata::new().robots(
                        Robots::new().index(false).follow(false),
                    ),
                },
            )
        },
    ))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let found = docs().page(slug(cx));
    let body = TrustedHtml(found.map(|p| p.html.clone()).unwrap_or_default());
    Ok(view! {
        if let Some(page) = found {
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
        } else {
            (StatusCode::NOT_FOUND)
            leaf_chamber(missing: true)
        }
    })
}
