//! The documentation: the repository's Markdown files (`docs/`, `docs/pt-BR/`) rendered once at startup, plus a generated parameter
//! reference. Math is written as `$…$` / `$$…$$` and becomes MathML on the server, so the pages need no script and no font files.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use pulldown_latex::{
    Parser as LatexParser, Storage,
    config::{DisplayMode, RenderConfig},
    push_mathml,
};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::OnceLock;
use tapiti_schema::{FieldKind, ParamGroup};

pub const GITHUB_BLOB: &str = "https://github.com/Lucas-BRT/tapiti/blob/main";

/// A documentation page: where its Markdown lives, in each language (`None` falls back to English).
struct Source {
    slug: &'static str,
    en: &'static str,
    pt: Option<&'static str>,
    /// Path under `docs/`, for the "edit on GitHub" link.
    file: &'static str,
}

const SOURCES: &[Source] = &[
    Source {
        slug: "getting-started",
        en: include_str!("../../../docs/getting-started.md"),
        pt: Some(include_str!("../../../docs/pt-BR/getting-started.md")),
        file: "getting-started.md",
    },
    Source {
        slug: "model",
        en: include_str!("../../../docs/dungeon-model.md"),
        pt: Some(include_str!("../../../docs/pt-BR/dungeon-model.md")),
        file: "dungeon-model.md",
    },
    Source {
        slug: "parameters",
        en: "",
        pt: None,
        file: "",
    },
    Source {
        slug: "performance",
        en: include_str!("../../../docs/performance-memory-review.md"),
        pt: Some(include_str!(
            "../../../docs/pt-BR/performance-memory-review.md"
        )),
        file: "performance-memory-review.md",
    },
    Source {
        slug: "desktop",
        en: include_str!("../../../docs/desktop-generation-rendering.md"),
        pt: Some(include_str!(
            "../../../docs/pt-BR/desktop-generation-rendering.md"
        )),
        file: "desktop-generation-rendering.md",
    },
    Source {
        slug: "roadmap",
        en: include_str!("../../../docs/roadmap.md"),
        pt: Some(include_str!("../../../docs/pt-BR/roadmap.md")),
        file: "roadmap.md",
    },
];

#[derive(Clone, Debug)]
pub struct TocEntry {
    pub level: u8,
    pub id: String,
    pub text: String,
}

/// One heading and the text under it, for search.
#[derive(Clone, Debug)]
pub struct Section {
    pub heading: String,
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub slug: &'static str,
    pub title: String,
    pub html: String,
    pub toc: Vec<TocEntry>,
    pub sections: Vec<Section>,
    /// Number of math spans in the source and number of `<math>` elements produced (they must match).
    #[allow(dead_code)] // read by the tests that pin the math pipeline
    pub math_spans: usize,
    #[allow(dead_code)]
    pub math_rendered: usize,
    /// True when the page has no translation and shows the English text.
    pub fallback: bool,
    /// Where to read or edit the source on GitHub.
    pub source_url: String,
}

pub struct Docs {
    by_locale: HashMap<&'static str, Vec<Page>>,
}

pub const LOCALES: [&str; 2] = ["en", "pt-BR"];

pub fn docs() -> &'static Docs {
    static DOCS: OnceLock<Docs> = OnceLock::new();
    DOCS.get_or_init(|| {
        let mut by_locale = HashMap::new();
        for locale in LOCALES {
            let pages = SOURCES.iter().map(|source| build(source, locale)).collect();
            by_locale.insert(locale, pages);
        }
        Docs { by_locale }
    })
}

impl Docs {
    pub fn pages(&self, locale: &str) -> &[Page] {
        self.by_locale.get(locale).map_or(&[], Vec::as_slice)
    }

    pub fn page(&self, locale: &str, slug: &str) -> Option<&Page> {
        self.pages(locale).iter().find(|p| p.slug == slug)
    }
}

fn build(source: &Source, locale: &'static str) -> Page {
    let portuguese = locale == "pt-BR";
    if source.slug == "parameters" {
        return parameters_page(locale);
    }
    let (markdown, fallback) = match (portuguese, source.pt) {
        (true, Some(pt)) => (pt, false),
        (true, None) => (source.en, true),
        _ => (source.en, false),
    };
    let mut page = render(markdown, source.slug, locale);
    page.fallback = fallback;
    let dir = if portuguese && !fallback {
        "pt-BR/"
    } else {
        ""
    };
    page.source_url = format!("{GITHUB_BLOB}/docs/{dir}{}", source.file);
    page
}

// ---- Markdown ------------------------------------------------------------------------------------

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
}

/// MathML for a TeX formula, or `None` when it does not parse.
fn mathml(tex: &str, display: bool) -> Option<String> {
    let storage = Storage::new();
    let mut events = Vec::new();
    for event in LatexParser::new(tex, &storage) {
        events.push(event.ok()?);
    }
    let mut out = String::new();
    let config = RenderConfig {
        display_mode: if display {
            DisplayMode::Block
        } else {
            DisplayMode::Inline
        },
        annotation: Some(tex),
        ..RenderConfig::default()
    };
    push_mathml(
        &mut out,
        events.into_iter().map(Ok::<_, std::convert::Infallible>),
        config,
    )
    .ok()?;
    Some(out)
}

/// The Greek letters the docs use, so that searching for "β" finds a section that only writes `\beta`.
fn greek_aliases(tex: &str) -> String {
    const LETTERS: &[(&str, &str)] = &[
        ("\\alpha", "α"),
        ("\\beta", "β"),
        ("\\gamma", "γ"),
        ("\\delta", "δ"),
        ("\\xi", "ξ"),
        ("\\rho", "ρ"),
        ("\\sigma", "σ"),
        ("\\theta", "θ"),
        ("\\lambda", "λ"),
        ("\\varphi", "φ"),
        ("\\omega", "ω"),
        ("\\Lambda", "Λ"),
        ("\\Phi", "Φ"),
        ("\\Pi", "Π"),
        ("\\iota", "ι"),
        ("\\tau", "τ"),
        ("\\kappa", "κ"),
        ("\\ell", "ℓ"),
    ];
    LETTERS
        .iter()
        .filter(|(command, _)| tex.contains(command))
        .map(|(_, letter)| *letter)
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Rewrites a link of the Markdown sources: page slugs become routes, repository paths become GitHub URLs.
fn rewrite_link(dest: &str, locale: &str) -> String {
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with('#')
        || dest.starts_with("mailto:")
    {
        return dest.to_string();
    }
    let trimmed = dest.trim_start_matches("../");
    if trimmed.starts_with("crates/")
        || trimmed.starts_with("apps/")
        || trimmed.starts_with("docs/")
        || trimmed.starts_with("src/")
    {
        return format!("{GITHUB_BLOB}/{trimmed}");
    }
    let (slug, fragment) = dest.split_once('#').map_or((dest, ""), |(s, f)| (s, f));
    if SOURCES.iter().any(|s| s.slug == slug) {
        let mut url = format!("/{locale}/docs/{slug}");
        if !fragment.is_empty() {
            let _ = write!(url, "#{fragment}");
        }
        return url;
    }
    dest.to_string()
}

fn render(markdown: &str, slug: &'static str, locale: &str) -> Page {
    let mut math_spans = 0;
    let mut math_rendered = 0;
    let mut events: Vec<Event> = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        events.push(match event {
            Event::InlineMath(tex) | Event::DisplayMath(tex)
                if {
                    math_spans += 1;
                    false
                } =>
            {
                Event::Text(tex)
            }
            Event::InlineMath(tex) => match mathml(&tex, false) {
                Some(m) => {
                    math_rendered += 1;
                    Event::Html(format!("{m}\u{200b}").into())
                }
                None => Event::Html(
                    format!("<code class=\"math-error\">{}</code>", escape(&tex)).into(),
                ),
            },
            Event::DisplayMath(tex) => match mathml(&tex, true) {
                Some(m) => {
                    math_rendered += 1;
                    Event::Html(
                        format!("<div class=\"math-display\" tabindex=\"0\">{m}</div>").into(),
                    )
                }
                None => {
                    Event::Html(format!("<pre class=\"math-error\">{}</pre>", escape(&tex)).into())
                }
            },
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Link {
                link_type,
                dest_url: CowStr::from(rewrite_link(&dest_url, locale)),
                title,
                id,
            }),
            other => other,
        });
    }
    // Search text: plain text per section, plus the TeX of math (with Greek letters spelled as letters).
    let mut title = String::new();
    let mut toc = Vec::new();
    let mut sections: Vec<Section> = vec![Section {
        heading: String::new(),
        id: String::new(),
        text: String::new(),
    }];
    let mut used_ids: HashMap<String, usize> = HashMap::new();
    let mut out_events: Vec<Event> = Vec::new();
    let mut i = 0;
    let mut raw_math = markdown_math(markdown).into_iter();
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let level_number = heading_number(*level);
                let mut j = i + 1;
                let mut text = String::new();
                while j < events.len() && !matches!(events[j], Event::End(TagEnd::Heading(_))) {
                    if let Event::Text(t) | Event::Code(t) = &events[j] {
                        text.push_str(t);
                    }
                    j += 1;
                }
                let base = slugify(&text);
                let count = used_ids.entry(base.clone()).or_insert(0);
                let id = if *count == 0 {
                    base.clone()
                } else {
                    format!("{base}-{count}")
                };
                *count += 1;
                if level_number == 1 && title.is_empty() {
                    title = text.clone();
                    out_events.push(Event::Html(format!("<h1 id=\"{id}\">").into()));
                } else {
                    out_events.push(Event::Html(format!("<h{level_number} id=\"{id}\">").into()));
                }
                out_events.extend(events[i + 1..j].iter().cloned());
                if level_number > 1 {
                    out_events.push(Event::Html(
                        format!("<a class=\"anchor\" href=\"#{id}\" aria-hidden=\"true\" tabindex=\"-1\">#</a>")
                            .into(),
                    ));
                    toc.push(TocEntry {
                        level: level_number,
                        id: id.clone(),
                        text: text.clone(),
                    });
                    sections.push(Section {
                        heading: text,
                        id,
                        text: String::new(),
                    });
                }
                out_events.push(Event::Html(format!("</h{level_number}>").into()));
                i = j + 1;
            }
            event => {
                let section = sections.last_mut().expect("there is always a section");
                match event {
                    Event::Text(t) | Event::Code(t) => {
                        section.text.push_str(t);
                        section.text.push(' ');
                    }
                    Event::SoftBreak | Event::HardBreak => section.text.push(' '),
                    Event::Html(h) if h.contains("<math") => {
                        if let Some(tex) = raw_math.next() {
                            section.text.push_str(&greek_aliases(&tex));
                            section.text.push(' ');
                        }
                    }
                    _ => {}
                }
                out_events.push(event.clone());
                i += 1;
            }
        }
    }
    let mut html_out = String::new();
    html::push_html(&mut html_out, out_events.into_iter());
    let html_out = html_out
        .replace(
            "<table>",
            "<div class=\"table-wrap\" tabindex=\"0\"><table>",
        )
        .replace("<pre>", "<pre tabindex=\"0\">")
        .replace("</table>", "</table></div>");
    Page {
        slug,
        title,
        html: html_out,
        toc,
        sections,
        math_spans,
        math_rendered,
        fallback: false,
        source_url: String::new(),
    }
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// The TeX source of each math span, in document order (used only to index the Greek letters for search).
fn markdown_math(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, options())
        .filter_map(|event| match event {
            Event::InlineMath(tex) | Event::DisplayMath(tex) => Some(tex.to_string()),
            _ => None,
        })
        .collect()
}

// ---- The parameter reference ------------------------------------------------------------------------

fn parameters_page(locale: &'static str) -> Page {
    let portuguese = locale == "pt-BR";
    let text = |en: &'static str, pt: &'static str| if portuguese { pt } else { en };
    let title = text("Parameters", "Parâmetros").to_string();
    let mut html = String::new();
    let _ = write!(html, "<h1 id=\"parameters\">{title}</h1>");
    let _ = write!(
        html,
        "<p>{}</p>",
        text(
            "Every parameter of Λ, as the desktop app and the lab show them. Names in code are the JSON field names, so a Λ copied from one frontend loads in the other. Pass a path like <code>growth.rooms</code> to read or set it.",
            "Todos os parâmetros de Λ, como o app desktop e o laboratório os mostram. Os nomes em código são os campos do JSON, então um Λ copiado de um frontend carrega no outro. Um caminho como <code>growth.rooms</code> lê ou define o parâmetro."
        )
    );
    let mut toc = Vec::new();
    let mut sections = vec![Section {
        heading: String::new(),
        id: String::new(),
        text: title.clone(),
    }];
    for group in tapiti_schema::groups() {
        let group_title = group.title.get(portuguese).to_string();
        let id = slugify(&group.id);
        let _ = write!(
            html,
            "<h2 id=\"{id}\">{}<a class=\"anchor\" href=\"#{id}\" aria-hidden=\"true\" tabindex=\"-1\">#</a></h2>",
            escape(&group_title)
        );
        let _ = write!(html, "<p>{}</p>", escape(group.intro.get(portuguese)));
        group_table(&mut html, &group, portuguese);
        let mut body = group.intro.get(portuguese).to_string();
        for field in &group.fields {
            let _ = write!(
                body,
                " {} {} {}",
                field.path,
                field.label.get(portuguese),
                field.help.get(portuguese)
            );
        }
        toc.push(TocEntry {
            level: 2,
            id: id.clone(),
            text: group_title.clone(),
        });
        sections.push(Section {
            heading: group_title,
            id,
            text: body,
        });
    }
    Page {
        slug: "parameters",
        title,
        html,
        toc,
        sections,
        math_spans: 0,
        math_rendered: 0,
        fallback: false,
        source_url: format!("{GITHUB_BLOB}/crates/tapiti-schema/src/lib.rs"),
    }
}

fn group_table(html: &mut String, group: &ParamGroup, portuguese: bool) {
    let head = if portuguese {
        ["Parâmetro", "Nome", "Valores", "O que faz"]
    } else {
        ["Parameter", "Name", "Values", "What it does"]
    };
    let _ = write!(
        html,
        "<div class=\"table-wrap\" tabindex=\"0\"><table><thead><tr>"
    );
    for h in head {
        let _ = write!(html, "<th>{h}</th>");
    }
    let _ = write!(html, "</tr></thead><tbody>");
    for field in &group.fields {
        let values = match &field.kind {
            FieldKind::Number { min, max, integer } => {
                if *integer {
                    format!("{min:.0} – {max:.0}")
                } else {
                    format!("{min} – {max}")
                }
            }
            FieldKind::Boolean => {
                if portuguese {
                    "ligado / desligado".into()
                } else {
                    "on / off".into()
                }
            }
            FieldKind::Choice(options) => options
                .iter()
                .map(|(_, name)| name.get(portuguese))
                .collect::<Vec<_>>()
                .join(", "),
        };
        let _ = write!(
            html,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&field.path),
            escape(field.label.get(portuguese)),
            escape(&values),
            escape(field.help.get(portuguese))
        );
    }
    let _ = write!(html, "</tbody></table></div>");
}

// ---- Search ------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Hit {
    pub slug: &'static str,
    pub page_title: String,
    pub heading: String,
    pub id: String,
    pub snippet: String,
}

/// Longest query accepted, in characters.
pub const MAX_QUERY: usize = 80;

/// Sections containing every word of `query`, best first. Plain substring search, case-insensitive.
pub fn search(locale: &str, query: &str) -> Vec<Hit> {
    let query: String = query.chars().take(MAX_QUERY).collect();
    let words: Vec<String> = query
        .split_whitespace()
        .take(5)
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, Hit)> = Vec::new();
    for page in docs().pages(locale) {
        for section in &page.sections {
            let heading = section.heading.to_lowercase();
            let body = section.text.to_lowercase();
            if !words
                .iter()
                .all(|w| heading.contains(w) || body.contains(w))
            {
                continue;
            }
            let score: usize = words
                .iter()
                .map(|w| 5 * heading.matches(w).count() + body.matches(w).count())
                .sum();
            scored.push((
                score,
                Hit {
                    slug: page.slug,
                    page_title: page.title.clone(),
                    heading: if section.heading.is_empty() {
                        page.title.clone()
                    } else {
                        section.heading.clone()
                    },
                    id: section.id.clone(),
                    snippet: snippet(&section.text, &words[0]),
                },
            ));
        }
    }
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().take(30).map(|(_, hit)| hit).collect()
}

/// About 160 characters of `text` around the first occurrence of `word`.
fn snippet(text: &str, word: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let needle: Vec<char> = word.chars().collect();
    let at = if lower.len() == chars.len() {
        (0..lower.len().saturating_sub(needle.len().saturating_sub(1)))
            .find(|&i| lower[i..].starts_with(&needle))
            .unwrap_or(0)
    } else {
        0
    };
    let start = at.saturating_sub(60);
    let end = (start + 160).min(chars.len());
    let mut out: String = chars[start..end]
        .iter()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if start > 0 {
        out.insert(0, '…');
    }
    if end < chars.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_math_span_becomes_mathml() {
        for locale in LOCALES {
            for page in docs().pages(locale) {
                let counted = page.html.matches("<math").count();
                assert_eq!(
                    page.math_spans, page.math_rendered,
                    "{locale}/{}: a formula failed to parse",
                    page.slug
                );
                assert_eq!(
                    page.math_spans, counted,
                    "{locale}/{}: math spans and <math> elements differ",
                    page.slug
                );
                assert!(!page.html.contains("math-error"), "{locale}/{}", page.slug);
            }
        }
    }

    #[test]
    fn the_model_page_has_all_its_formulas() {
        let page = docs().page("en", "model").unwrap();
        assert!(page.math_spans > 200, "{}", page.math_spans);
    }

    #[test]
    fn heading_ids_are_unique_and_links_resolve() {
        for locale in LOCALES {
            for page in docs().pages(locale) {
                let mut ids = std::collections::HashSet::new();
                for entry in &page.toc {
                    assert!(
                        ids.insert(entry.id.clone()),
                        "{locale}/{}: duplicate id {}",
                        page.slug,
                        entry.id
                    );
                    assert!(page.html.contains(&format!("id=\"{}\"", entry.id)));
                }
                for href in page
                    .html
                    .split("href=\"")
                    .skip(1)
                    .map(|s| s.split('"').next().unwrap())
                {
                    if let Some(rest) = href.strip_prefix(&format!("/{locale}/docs/")) {
                        let (slug, fragment) = rest.split_once('#').unwrap_or((rest, ""));
                        let target = docs().page(locale, slug).unwrap_or_else(|| {
                            panic!("{locale}/{}: broken link {href}", page.slug)
                        });
                        assert!(
                            fragment.is_empty()
                                || target.html.contains(&format!("id=\"{fragment}\"")),
                            "{locale}/{}: missing anchor in {href}",
                            page.slug
                        );
                    } else if let Some(fragment) = href.strip_prefix('#') {
                        assert!(
                            page.html.contains(&format!("id=\"{fragment}\"")),
                            "{locale}/{}: missing anchor {href}",
                            page.slug
                        );
                    } else {
                        assert!(
                            href.starts_with("https://")
                                || href.starts_with("mailto:")
                                || href.starts_with("http://localhost"),
                            "{locale}/{}: unresolved link {href}",
                            page.slug
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn titles_exist_and_translations_differ() {
        for page in docs().pages("en") {
            assert!(!page.title.is_empty(), "{}", page.slug);
        }
        let en = docs().page("en", "roadmap").unwrap();
        let pt = docs().page("pt-BR", "roadmap").unwrap();
        assert_ne!(en.title, pt.title);
        assert!(!pt.fallback);
    }

    #[test]
    fn search_finds_greek_letters_and_words_in_both_languages() {
        assert!(!search("en", "β").is_empty());
        assert!(!search("pt-BR", "β").is_empty());
        assert!(!search("en", "corridor").is_empty());
        assert!(!search("pt-BR", "corredor").is_empty());
        assert!(search("en", "xyzzyplugh").is_empty());
        assert!(search("en", &"a".repeat(500)).len() <= 30);
    }

    #[test]
    fn the_parameter_reference_lists_every_field() {
        let page = docs().page("en", "parameters").unwrap();
        for group in tapiti_schema::groups() {
            for field in group.fields {
                assert!(
                    page.html.contains(&format!("<code>{}</code>", field.path)),
                    "{}",
                    field.path
                );
            }
        }
    }
}
