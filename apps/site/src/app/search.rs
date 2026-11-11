use topcoat::{
    Result,
    context::Cx,
    metadata::Metadata,
    router::{href, page, query_params},
    view::{View, view},
};

use super::docs::{self, slug};
use crate::{
    app::t,
    docs::{MAX_QUERY, search},
};

#[query_params(error = bad_request)]
struct Query {
    q: Option<String>,
}

/// Documentation search: a plain GET form, so it works without a script and can be linked.
#[page(with(Metadata::new().title(t!(search_title))))]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let query: String = query_params::<Query>(cx)?
        .q
        .clone()
        .unwrap_or_default()
        .chars()
        .take(MAX_QUERY)
        .collect();
    let hits = search(&query);
    let searched = !query.trim().is_empty();
    let count = hits.len();
    Ok(view! {
        docs::frame(
            current: "",
            <div class="docs-content search-page">
                <h1>(t!(search_title))</h1>
                <form role="search" method="get" action=(href!(page))>
                    <label class="visually-hidden" for="search-q">
                        (t!(docs_search_label))
                    </label>
                    <input
                        id="search-q"
                        type="search"
                        name="q"
                        maxlength="80"
                        value=(query.as_str())
                        placeholder=(t!(docs_search_placeholder))
                    >
                    <button class="button primary" type="submit">
                        (t!(search_button))
                    </button>
                </form>
                if searched {
                    if count == 0 {
                        <p class="search-none">
                            (t!(search_none, query: query.as_str()))
                        </p>
                    } else {
                        <p class="search-count">(t!(search_count, count))</p>
                        <ol class="search-results">
                            for hit in hits {
                                <li>
                                    <a
                                        href=(href!(slug::_page::page, slug: hit.slug).fragment(
                                            hit.id.as_str(),
                                        ))
                                    >
                                        <span class="hit-page">(hit.page_title.as_str())</span>
                                        <span class="hit-heading">(hit.heading.as_str())</span>
                                    </a>
                                    <p>(hit.snippet.as_str())</p>
                                </li>
                            }
                        </ol>
                    }
                }
            </div>
        )
    })
}
