pub mod _page;
pub mod docs;
pub mod lab;
pub mod leaf;
mod legacy;
pub mod search;
mod walk;

use std::path::PathBuf;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt, asset},
    context::{BaseUrl, Cx},
    font::{Font, fontsource::fontsource_font},
    l10n::{fluent::messages, locale},
    metadata::{Metadata, OpenGraph, Url},
    router::{
        Body, HeaderName, HeaderValue, Next, Router, RouterBuilderDirectoryExt,
        RouterBuilderDiscoverExt, Slot,
        content::sitemap::{ChangeFrequency, Sitemap, SitemapUrl},
        header::CONTENT_TYPE,
        href, layer, layout, module_router,
        response::Response,
        route,
    },
    view::{View, component, view},
};

// Loads `src/l10n/en.ftl` and creates `t!`. The site is English only.
messages!("./l10n/{locale}.ftl");

pub fn router() -> Router {
    // Measured now, at startup, so the first visitor does not wait for it.
    crate::floor::speed();
    module_router!()
        .with(BaseUrl::new(site_url()).expect("TAPITI_SITE_URL must be an absolute URL such as https://tapiti.example"))
        .discover()
        .assets(AssetBundle::load().expect("asset bundle missing: run `topcoat dev`, or `topcoat asset bundle --package tapiti-site` after building"))
        // The lab's WebAssembly module and its glue live at fixed paths: the glue finds its `.wasm` next to itself, so they cannot be hashed assets.
        .serve_dir("/lab/pkg/{*file}", pkg_dir())
        .build()
}

/// The public address of the site (for canonical links and social previews): `TAPITI_SITE_URL`, else the local one.
pub(crate) fn site_url() -> String {
    std::env::var("TAPITI_SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into())
}

/// Where the WebAssembly module of the lab is: `TAPITI_LAB_PKG`, else the nearest `apps/site/pkg` above the executable (what `scripts/build-wasm.sh` fills).
fn pkg_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TAPITI_LAB_PKG") {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.ancestors()
                .map(|dir| dir.join("apps/site/pkg"))
                .find(|dir| dir.join("tapiti.js").is_file())
        })
        .unwrap_or_else(|| PathBuf::from("apps/site/pkg"))
}

/// Browsers ask for this path on their own, whatever the page links to.
#[route(GET "/favicon.ico")]
async fn favicon(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 1], Vec<u8>)> {
    let icon = include_bytes!("../assets/brand/favicon.ico").to_vec();
    Ok((
        [(CONTENT_TYPE, HeaderValue::from_static("image/x-icon"))],
        icon,
    ))
}

/// Tells crawlers where the sitemap is. The sitemap needs absolute URLs, which come from `TAPITI_SITE_URL`.
#[route(GET "/robots.txt")]
async fn robots(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 1], String)> {
    let text = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        site_url().trim_end_matches('/')
    );
    Ok((
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        text,
    ))
}

/// Every page: the landing page, the lab and each documentation page.
#[route(GET "/sitemap.xml")]
async fn sitemap(cx: &Cx) -> Result<Sitemap> {
    let mut sitemap = Sitemap::new()
        .url(
            SitemapUrl::new("/")
                .change_frequency(ChangeFrequency::Monthly)
                .priority(1.0),
        )
        .url(
            SitemapUrl::new("/lab")
                .change_frequency(ChangeFrequency::Monthly)
                .priority(0.8),
        );
    for page in crate::docs::docs().pages() {
        sitemap = sitemap.url(
            SitemapUrl::new(format!("/docs/{}", page.slug))
                .change_frequency(ChangeFrequency::Monthly)
                .priority(0.7),
        );
    }
    Ok(sitemap)
}

/// Liveness probe for the container.
#[route(GET "/healthz")]
async fn healthz(cx: &Cx) -> Result<&'static str> {
    Ok("ok")
}

/// Container health probe, run as `tapiti-site --healthcheck` (the image has no curl): asks this machine for `/healthz`.
pub fn healthcheck() -> bool {
    use std::io::{Read, Write};
    use std::time::Duration;
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let timeout = Some(Duration::from_secs(3));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    if stream
        .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).is_ok() && head.ends_with(b" 200")
}

const GITHUB: &str = "https://github.com/Lucas-BRT/tapiti";
const GITHUB_ISSUES: &str = "https://github.com/Lucas-BRT/tapiti/issues";

// Self-hosted: the font files are bundled as assets and served by this app.
// rustfmt and `topcoat fmt` wrap this macro differently; keep the Topcoat layout.
#[rustfmt::skip]
const GRENZE: Font =
    fontsource_font!(
        GRENZE,
        weight: [600, 700],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const ALEGREYA_SANS: Font =
    fontsource_font!(
        ALEGREYA_SANS,
        weight: [400, 500, 700],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const ALEGREYA_SANS_ITALIC: Font =
    fontsource_font!(
        ALEGREYA_SANS,
        weight: [400],
        style: Italic,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const JETBRAINS_MONO: Font =
    fontsource_font!(
        JETBRAINS_MONO,
        weight: [400],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );

// Every request is rendered in English: Topcoat's `t!` needs a language in the context.
#[layer]
async fn language(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let cx = cx.with(locale!("en"));
    next.run(&cx, body).await
}

#[layout(
    with(Metadata::new()
        .title(t!(title))
        .description(t!(description))
        .theme_color("#0b0e12")
        .canonical(Url::Current)
        .open_graph(
            OpenGraph::new().site_name("Tapiti").image(asset!("assets/brand/og.png")),
        ))
)]
async fn layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta name="color-scheme" content="dark">
                topcoat::metadata::tags()
                topcoat::font::link(font: GRENZE)
                topcoat::font::link(font: ALEGREYA_SANS)
                topcoat::font::link(font: ALEGREYA_SANS_ITALIC)
                topcoat::font::link(font: JETBRAINS_MONO)
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
                <script src=(asset!("assets/site.js")) defer="defer"></script>
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
            <a class="brand" href=(href!(_page::page))>
                <img
                    src=(asset!("assets/brand/mark.svg"))
                    alt=""
                    width="28"
                    height="28"
                >
                "tapiti"
            </a>
            <nav class="site-nav" aria-label=(t!(nav_label))>
                <a href=(href!(docs::_page::page))>(t!(nav_docs))</a>
                <a href=(href!(lab::page))>(t!(nav_lab))</a>
                <a href=(GITHUB)>"GitHub"</a>
            </nav>
        </header>
    })
}

#[component]
async fn footer(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <footer class="site-footer">
            <div class="footer-about">
                <a class="brand" href=(href!(_page::page))>
                    <img
                        src=(asset!("assets/brand/mark.svg"))
                        alt=""
                        width="28"
                        height="28"
                    >
                    "tapiti"
                </a>
                <p>(t!(footer_license))</p>
            </div>
            <nav aria-label=(t!(footer_explore))>
                <h2>(t!(footer_explore))</h2>
                <a href=(href!(docs::_page::page))>(t!(nav_docs))</a>
                <a href=(href!(lab::page))>(t!(nav_lab))</a>
            </nav>
            <nav aria-label=(t!(footer_project))>
                <h2>(t!(footer_project))</h2>
                <a href=(GITHUB)>(t!(footer_source))</a>
                <a href=(GITHUB_ISSUES)>(t!(footer_issues))</a>
            </nav>
        </footer>
    })
}
