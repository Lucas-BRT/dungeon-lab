//! Old addresses: the site used to put the language first (`/en/docs/model`, `/pt-BR`). They move to the flat URL for good.

use topcoat::{
    Result,
    context::Cx,
    router::{
        error::{not_found, redirect_permanent},
        request::uri,
        route,
    },
};

/// Where an old `/en…` or `/pt-BR…` address now lives, with its query. `None` when the rest of the path could point away from this site
/// (`/en//evil.example` would become the protocol-relative URL `//evil.example`).
fn flat(path: &str, query: Option<&str>) -> Option<String> {
    let rest = ["/en", "/pt-BR"]
        .iter()
        .find_map(|prefix| path.strip_prefix(prefix))?;
    let rest = if rest.is_empty() { "/" } else { rest };
    if !rest.starts_with('/') || rest.starts_with("//") || rest.contains('\\') {
        return None;
    }
    Some(match query {
        Some(query) => format!("{rest}?{query}"),
        None => rest.to_string(),
    })
}

fn moved(cx: &Cx) -> Result<()> {
    let uri = uri(cx);
    match flat(uri.path(), uri.query()) {
        Some(target) => Err(redirect_permanent(target).into()),
        None => Err(not_found().into()),
    }
}

#[route(GET "/en")]
async fn en(cx: &Cx) -> Result<()> {
    moved(cx)
}

#[route(GET "/en/{*rest}")]
async fn en_rest(cx: &Cx) -> Result<()> {
    moved(cx)
}

#[route(GET "/pt-BR")]
async fn pt(cx: &Cx) -> Result<()> {
    moved(cx)
}

#[route(GET "/pt-BR/{*rest}")]
async fn pt_rest(cx: &Cx) -> Result<()> {
    moved(cx)
}

#[cfg(test)]
mod tests {
    use super::flat;

    #[test]
    fn old_addresses_move_to_the_flat_ones() {
        assert_eq!(flat("/en", None).as_deref(), Some("/"));
        assert_eq!(flat("/pt-BR", None).as_deref(), Some("/"));
        assert_eq!(flat("/en/lab", None).as_deref(), Some("/lab"));
        assert_eq!(
            flat("/en/docs/model", Some("x=1")).as_deref(),
            Some("/docs/model?x=1")
        );
        assert_eq!(
            flat("/pt-BR/search", Some("q=β")).as_deref(),
            Some("/search?q=β")
        );
    }

    #[test]
    fn nothing_can_redirect_away_from_the_site() {
        assert_eq!(flat("/en//evil.example", None), None);
        assert_eq!(flat("/pt-BR//evil.example/x", Some("a=b")), None);
        assert_eq!(flat("/en/\\evil.example", None), None);
        assert_eq!(flat("/english", None), None);
        assert_eq!(flat("/docs/model", None), None);
    }
}
