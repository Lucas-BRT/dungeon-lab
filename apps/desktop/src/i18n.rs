//! Minimal i18n: the English text is the key, so the source reads naturally and English is the default.
//! A language is a table of `(english, translation)` pairs (see `pt.rs`); a missing entry falls back to English.
//!
//! * `t!("Text")` returns the translation of a literal.
//! * `tf!("Hello {name}", name = x)` translates the template, then fills the named placeholders.

mod pt;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Pt,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Pt];

    /// Each language is named in itself.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Pt => "Português",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn is_pt() -> bool {
    get() == Lang::Pt
}

pub fn get() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Pt,
        _ => Lang::En,
    }
}

fn table(lang: Lang) -> Option<&'static HashMap<&'static str, &'static str>> {
    static PT: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    match lang {
        Lang::En => None,
        Lang::Pt => Some(PT.get_or_init(|| pt::PT.iter().copied().collect())),
    }
}

pub fn tr(key: &str) -> &str {
    table(get()).and_then(|t| t.get(key).copied()).unwrap_or(key)
}

pub fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

macro_rules! t {
    ($key:literal) => {
        $crate::i18n::tr($key)
    };
}

macro_rules! tf {
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::fill($crate::i18n::tr($key), &[$((stringify!($name), $value.to_string())),+])
    };
}

/// Decimal separator of the active language.
pub fn decimal(text: String) -> String {
    match get() {
        Lang::En => text,
        Lang::Pt => text.replace('.', ","),
    }
}

#[cfg(test)]
mod tests {
    use super::pt::PT;
    use std::collections::HashSet;

    /// Every `t!("…")` / `tf!("…", …)` literal in the sources.
    fn keys() -> HashSet<String> {
        let mut found = HashSet::new();
        for source in [
            include_str!("main.rs"),
            include_str!("panels.rs"),
            include_str!("params_ui.rs"),
            include_str!("map.rs"),
        ] {
            for marker in ["t!(", "tf!("] {
                let mut rest = source;
                while let Some(at) = rest.find(marker) {
                    // `at` may be the tail of `fmt!(` or `macro_rules! t!(`; only a standalone macro counts.
                    let before = rest[..at].chars().last();
                    rest = &rest[at + marker.len()..];
                    let trimmed = rest.trim_start();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') || !trimmed.starts_with('"') {
                        continue;
                    }
                    let mut text = String::new();
                    let mut chars = trimmed[1..].chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => match chars.next() {
                                Some('n') => text.push('\n'),
                                Some(other) => text.push(other),
                                None => break,
                            },
                            c => text.push(c),
                        }
                    }
                    found.insert(text);
                }
            }
        }
        found
    }

    fn placeholders(text: &str) -> Vec<String> {
        let mut names: Vec<String> = text
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_string()))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn portuguese_covers_every_key_and_nothing_else() {
        let used = keys();
        let table: HashSet<&str> = PT.iter().map(|(en, _)| *en).collect();
        assert_eq!(table.len(), PT.len(), "duplicate key in pt.rs");
        let missing: Vec<_> = used.iter().filter(|k| !table.contains(k.as_str())).collect();
        assert!(missing.is_empty(), "keys without a pt entry: {missing:#?}");
        let stale: Vec<_> = table.iter().filter(|k| !used.contains(**k)).collect();
        assert!(stale.is_empty(), "pt entries no longer used: {stale:#?}");
        for (en, pt) in PT {
            assert_eq!(placeholders(en), placeholders(pt), "placeholders differ: {en}");
        }
    }
}
