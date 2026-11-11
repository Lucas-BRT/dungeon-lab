pub mod _page;
pub mod slug;

use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    router::href,
    view::{Child, View, component, view},
};

use super::search;
use crate::{app::t, docs::docs};

/// The documentation's frame: the search box and the list of pages beside whatever `child` shows. `current` is the slug being read.
#[component]
pub async fn frame(cx: &Cx, current: &str, #[default] child: Child<'_>) -> Result<impl View> {
    let pages = docs().pages();
    Ok(view! {
        <link rel="stylesheet" href=(asset!("assets/docs.css"))>
        <div class="docs">
            <aside class="docs-nav">
                <form
                    class="docs-search"
                    role="search"
                    method="get"
                    action=(href!(search::page))
                >
                    <label class="visually-hidden" for="docs-q">
                        (t!(docs_search_label))
                    </label>
                    <input
                        id="docs-q"
                        type="search"
                        name="q"
                        maxlength="80"
                        placeholder=(t!(docs_search_placeholder))
                    >
                </form>
                <nav aria-label=(t!(docs_nav_label))>
                    <ul>
                        for page in pages {
                            <li>
                                <a
                                    href=(href!(slug::_page::page, slug: page.slug))
                                    if page.slug == current {
                                        aria-current="page"
                                    }
                                >
                                    (page.title.as_str())
                                </a>
                            </li>
                        }
                    </ul>
                </nav>
            </aside>
            <div class="docs-main">(child)</div>
        </div>
    })
}
