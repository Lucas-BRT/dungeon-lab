mod locale;

use std::path::PathBuf;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{BaseUrl, Cx},
    l10n::{accept_language, fluent::messages, locale, negotiate},
    router::{
        HeaderName, HeaderValue, Router, RouterBuilderDirectoryExt, RouterBuilderDiscoverExt,
        content::sitemap::{ChangeFrequency, Sitemap, SitemapUrl},
        error::{not_found, redirect},
        header::CONTENT_TYPE,
        href, module_router, page, route,
    },
    view::View,
};

// Loads `src/l10n/en.ftl`, `src/l10n/pt-BR.ftl`, and creates `t!`. English is the source language.
messages!("./l10n/{locale}.ftl");

pub fn router() -> Router {
    // Measured now, at startup, so the first visitor does not wait for it.
    crate::floor::speed();
    module_router!()
        .with(BaseUrl::new(site_url()).expect("TAPITI_SITE_URL must be an absolute URL such as https://tapiti.example"))
        .discover()
        .assets(AssetBundle::load().expect("asset bundle missing: run `topcoat dev`, or `topcoat asset bundle --package tapiti-site` after building"))
        // The browser lab keeps fixed URLs (it loads its own scripts and the WebAssembly module by relative path).
        .serve_dir("/lab/{*file}", lab_dir())
        .build()
}

/// The public address of the site (for canonical links and social previews): `TAPITI_SITE_URL`, else the local one.
fn site_url() -> String {
    std::env::var("TAPITI_SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into())
}

/// Where the lab's files live: `TAPITI_LAB_DIR`, else the nearest `web/` above the executable (the repository in development, `/app` in the container).
fn lab_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TAPITI_LAB_DIR") {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.ancestors()
                .map(|dir| dir.join("web"))
                .find(|dir| dir.join("index.html").is_file())
        })
        .unwrap_or_else(|| PathBuf::from("web"))
}

/// Sends the visitor from `/` to the language that best fits their browser.
#[page]
async fn redirect_to_locale(cx: &Cx) -> Result<impl View> {
    let locale = negotiate(accept_language(cx), t::locales())
        .cloned()
        .unwrap_or(locale!("en"));
    Err::<(), _>(
        redirect(href!(locale::_page::page, locale::path_locale: locale).resolve(cx)).into(),
    )
}

/// The lab's page. It is served at `/lab` (the router drops trailing slashes), so a `<base>` makes its relative script paths resolve below `/lab/`.
#[route(GET "/lab")]
async fn lab(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 1], String)> {
    let page = tokio::fs::read_to_string(lab_dir().join("index.html"))
        .await
        .map_err(|_| not_found())?;
    let page = page.replacen("<head>", "<head>\n    <base href=\"/lab/\">", 1);
    Ok((
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        page,
    ))
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

/// Every page of every language: the landing page and each documentation page.
#[route(GET "/sitemap.xml")]
async fn sitemap(cx: &Cx) -> Result<Sitemap> {
    let mut sitemap = Sitemap::new();
    for locale in crate::docs::LOCALES {
        sitemap = sitemap.url(
            SitemapUrl::new(format!("/{locale}"))
                .change_frequency(ChangeFrequency::Monthly)
                .priority(1.0),
        );
        for page in crate::docs::docs().pages(locale) {
            sitemap = sitemap.url(
                SitemapUrl::new(format!("/{locale}/docs/{}", page.slug))
                    .change_frequency(ChangeFrequency::Monthly)
                    .priority(0.7),
            );
        }
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
