pub mod _page;
pub mod docs;
pub mod leaf;
pub mod search;

use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    font::{Font, fontsource::fontsource_font},
    l10n::{Locale, locale},
    metadata::{Metadata, OpenGraph},
    router::{
        Body, Next, Slot, error::not_found, href, layer, layout, module_param, response::Response,
    },
    view::{View, component, view},
};

use super::t;

const GITHUB: &str = "https://github.com/Lucas-BRT/tapiti";
const GITHUB_ISSUES: &str = "https://github.com/Lucas-BRT/tapiti/issues";

// Self-hosted: the font files are bundled as assets and served by this app.
// rustfmt and `topcoat fmt` wrap this macro differently; keep the Topcoat layout.
#[rustfmt::skip]
const FRAUNCES: Font =
    fontsource_font!(
        FRAUNCES,
        weight: [600],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const DM_SANS: Font =
    fontsource_font!(
        DM_SANS,
        weight: [400, 600],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );

// The language is the first part of the URL, as in /en or /pt-BR.
module_param!(pub path_locale: Locale else not_found);

// Sets the language of every page below. Each `t!` translates into it.
#[layer]
async fn layer(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let path_locale = path_locale(cx)?;
    // Any text can be typed into the URL, so reject the languages without messages.
    if !t::locales().contains(path_locale) {
        return Err(not_found().into());
    }
    let cx = cx.with(path_locale.clone());
    next.run(&cx, body).await
}

#[layout(
    with(Metadata::new()
        .title(t!(title))
        .description(t!(description))
        .theme_color("#15110e")
        .open_graph(
            OpenGraph::new().site_name("Tapiti").image(asset!("assets/brand/og.png")),
        ))
)]
async fn layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang=(locale(cx).id()) dir=(locale(cx).id().direction())>
            <head>
                <meta name="color-scheme" content="dark light">
                topcoat::metadata::tags()
                topcoat::font::link(font: FRAUNCES)
                topcoat::font::link(font: DM_SANS)
                <link
                    rel="icon"
                    href=(asset!("assets/brand/mark.svg"))
                    type="image/svg+xml"
                >
                <link rel="icon" href="/favicon.ico" sizes="32x32">
                <link
                    rel="apple-touch-icon"
                    href=(asset!("assets/brand/apple-touch-icon.png"))
                >
                <link rel="stylesheet" href=(asset!("assets/styles.css"))>
                topcoat::dev::script()
            </head>
            <body>
                <a class="skip-link" href="#main">(t!(skip_link))</a>
                header()
                <main id="main">(slot)</main>
                footer()
            </body>
        </html>
    })
}

#[component]
async fn header(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <header class="site-header">
            <a class="brand" href=(href!(_page::page, path_locale: locale(cx)))>
                "tapiti"
            </a>
            <nav class="site-nav" aria-label=(t!(nav_label))>
                <a href=(href!(docs::_page::page, path_locale: locale(cx)))>
                    (t!(nav_docs))
                </a>
                <a href=(href!(crate::app::lab))>(t!(nav_lab))</a>
                <a href=(GITHUB)>"GitHub"</a>
                <span class="lang">
                    for link_locale in t::locales() {
                        <a
                            href=(href!(_page::page, path_locale: link_locale))
                            lang=(link_locale.id())
                            hreflang=(link_locale.id())
                            if link_locale == locale(cx) {
                                aria-current="true"
                            }
                        >
                            (t!(language).resolve_in(link_locale))
                        </a>
                    }
                </span>
            </nav>
        </header>
    })
}

#[component]
async fn footer(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <footer class="site-footer">
            <span>(t!(footer_license))</span>
            <a href=(GITHUB)>(t!(footer_source))</a>
            <a href=(GITHUB_ISSUES)>(t!(footer_issues))</a>
        </footer>
    })
}
