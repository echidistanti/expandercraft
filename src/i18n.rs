//! User interface translations, based on [Fluent](https://projectfluent.org).
//!
//! Each language is a `locales/xx.ftl` file embedded in the executable. English (`en.ftl`) is the
//! reference: a message missing from another locale falls back to English, then to its id.
//!
//! * `tr("message-id")` returns a simple message as a `&'static str`.
//! * `tr!("message-id", name = value, ...)` formats a message with arguments (plurals included) as a `String`.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use parking_lot::Mutex;
use unic_langid::LanguageIdentifier;

use crate::prefs::Language;

/// Format a message with arguments: `tr!("msg-combos-imported", count = n)`.
macro_rules! tr {
    ($id:literal) => {
        $crate::i18n::tr($id)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::tr_args($id, &args)
    }};
}

/// A language the user interface is translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    It,
    Fr,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::En, Lang::It, Lang::Fr, Lang::Es];

    /// The BCP 47 language code.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::It => "it",
            Lang::Fr => "fr",
            Lang::Es => "es",
        }
    }

    /// The name of the language, written in that language.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::It => "Italiano",
            Lang::Fr => "Français",
            Lang::Es => "Español",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Lang::En => include_str!("../locales/en.ftl"),
            Lang::It => include_str!("../locales/it.ftl"),
            Lang::Fr => include_str!("../locales/fr.ftl"),
            Lang::Es => include_str!("../locales/es.ftl"),
        }
    }

    /// The translation matching a locale tag such as `fr-CA`, `es_ES.UTF-8` or `it`.
    pub fn from_locale(tag: &str) -> Option<Lang> {
        let primary = tag.split(['-', '_', '.', '@']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    fn index(self) -> usize {
        self as usize
    }
}

impl From<Language> for Option<Lang> {
    fn from(pref: Language) -> Self {
        match pref {
            Language::System => None,
            Language::English => Some(Lang::En),
            Language::Italian => Some(Lang::It),
            Language::French => Some(Lang::Fr),
            Language::Spanish => Some(Lang::Es),
        }
    }
}

struct Catalog {
    bundle: FluentBundle<FluentResource>,
    /// Simple messages already formatted, leaked once so they can be returned as `&'static str`.
    cache: Mutex<HashMap<&'static str, &'static str>>,
}

impl Catalog {
    fn load(lang: Lang) -> Self {
        let resource = FluentResource::try_new(lang.source().to_string()).unwrap_or_else(|(res, errors)| {
            log::error!("{}.ftl: {} syntax error(s), first: {:?}", lang.code(), errors.len(), errors.first());
            res
        });
        let langid: LanguageIdentifier = lang.code().parse().expect("valid language code");
        let mut bundle = FluentBundle::new_concurrent(vec![langid]);
        // the Unicode isolation marks around placeables are not rendered by egui
        bundle.set_use_isolating(false);
        if let Err(errors) = bundle.add_resource(resource) {
            log::error!("{}.ftl: {errors:?}", lang.code());
        }
        Self { bundle, cache: Mutex::new(HashMap::new()) }
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> Option<String> {
        let message = self.bundle.get_message(id)?;
        let pattern = message.value()?;
        let mut errors = Vec::new();
        let text = self.bundle.format_pattern(pattern, args, &mut errors);
        if !errors.is_empty() {
            log::warn!("message '{id}': {errors:?}");
        }
        Some(text.into_owned())
    }
}

static CATALOGS: OnceLock<Vec<Catalog>> = OnceLock::new();
static CURRENT: AtomicU8 = AtomicU8::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn catalog(lang: Lang) -> &'static Catalog {
    &CATALOGS.get_or_init(|| Lang::ALL.into_iter().map(Catalog::load).collect())[lang.index()]
}

/// The language matching the system locale, English if it is not translated.
pub fn system_language() -> Lang {
    sys_locale::get_locales().find_map(|l| Lang::from_locale(&l)).unwrap_or(Lang::En)
}

/// Select the language of the user interface.
pub fn set_language(pref: Language) {
    let lang = Option::<Lang>::from(pref).unwrap_or_else(system_language);
    if CURRENT.swap(lang.index() as u8, Ordering::Relaxed) != lang.index() as u8 {
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

/// The current language of the user interface.
pub fn current() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) as usize]
}

/// Incremented each time the language changes, so that cached texts (tray menu...) can be refreshed.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Translate a message without arguments.
pub fn tr(id: &'static str) -> &'static str {
    let lang = current();
    let cat = catalog(lang);
    if let Some(s) = cat.cache.lock().get(id) {
        return s;
    }
    let text = cat.format(id, None).or_else(|| {
        log::warn!("message '{id}' missing in {}.ftl", lang.code());
        (lang != Lang::En).then(|| catalog(Lang::En).format(id, None)).flatten()
    });
    let s: &'static str = match text {
        Some(t) => Box::leak(t.into_boxed_str()),
        None => id,
    };
    cat.cache.lock().insert(id, s);
    s
}

/// Translate a message with arguments. Prefer the `tr!` macro.
pub fn tr_args(id: &str, args: &FluentArgs) -> String {
    let lang = current();
    catalog(lang)
        .format(id, Some(args))
        .or_else(|| (lang != Lang::En).then(|| catalog(Lang::En).format(id, Some(args))).flatten())
        .unwrap_or_else(|| id.to_string())
}

/// Translate a message in a given language, for tests.
#[cfg(test)]
fn tr_in(lang: Lang, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    catalog(lang).format(id, args)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use fluent_syntax::ast;

    use super::*;

    /// Message id → names of the variables it uses, for one locale.
    fn messages(lang: Lang) -> BTreeMap<String, BTreeSet<String>> {
        let res = fluent_syntax::parser::parse(lang.source()).unwrap_or_else(|(_, e)| panic!("{}.ftl: {e:?}", lang.code()));
        let mut out = BTreeMap::new();
        for entry in res.body {
            if let ast::Entry::Message(m) = entry {
                let mut vars = BTreeSet::new();
                if let Some(p) = &m.value {
                    collect_vars(p, &mut vars);
                }
                assert!(out.insert(m.id.name.to_string(), vars).is_none(), "{}.ftl: duplicate {}", lang.code(), m.id.name);
            }
        }
        out
    }

    fn collect_vars(p: &ast::Pattern<&str>, vars: &mut BTreeSet<String>) {
        for el in &p.elements {
            if let ast::PatternElement::Placeable { expression } = el {
                collect_expr(expression, vars);
            }
        }
    }

    fn collect_expr(e: &ast::Expression<&str>, vars: &mut BTreeSet<String>) {
        match e {
            ast::Expression::Select { selector, variants } => {
                collect_inline(selector, vars);
                for v in variants {
                    collect_vars(&v.value, vars);
                }
            }
            ast::Expression::Inline(i) => collect_inline(i, vars),
        }
    }

    fn collect_inline(i: &ast::InlineExpression<&str>, vars: &mut BTreeSet<String>) {
        match i {
            ast::InlineExpression::VariableReference { id } => {
                vars.insert(id.name.to_string());
            }
            ast::InlineExpression::Placeable { expression } => collect_expr(expression, vars),
            _ => {}
        }
    }

    #[test]
    fn locales_parse_without_errors() {
        for lang in Lang::ALL {
            assert!(FluentResource::try_new(lang.source().to_string()).is_ok(), "{}.ftl has syntax errors", lang.code());
        }
    }

    #[test]
    fn locales_are_complete_and_consistent() {
        let reference = messages(Lang::En);
        for lang in [Lang::It, Lang::Fr, Lang::Es] {
            let other = messages(lang);
            let missing: Vec<_> = reference.keys().filter(|k| !other.contains_key(*k)).collect();
            let extra: Vec<_> = other.keys().filter(|k| !reference.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{}.ftl misses {missing:?}", lang.code());
            assert!(extra.is_empty(), "{}.ftl has unknown messages {extra:?}", lang.code());
            for (id, vars) in &reference {
                assert_eq!(vars, &other[id], "{}.ftl: variables of '{id}' differ from en.ftl", lang.code());
            }
        }
    }

    /// Every message used in the source code exists, and every message of en.ftl is used.
    #[test]
    fn messages_used_in_source_exist() {
        let reference = messages(Lang::En);
        let re_ids = ["tr(\"", "tr!(\""];
        let mut used = BTreeSet::new();
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut stack = vec![std::path::PathBuf::from(src)];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") || path.ends_with("i18n.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for pat in re_ids {
                    for (pos, _) in text.match_indices(pat) {
                        // ignore identifiers ending in "tr" (push_str...)
                        let prev = pos.checked_sub(1).map(|p| text.as_bytes()[p]);
                        if prev.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_') {
                            continue;
                        }
                        let rest = &text[pos + pat.len()..];
                        let id = &rest[..rest.find('"').unwrap()];
                        assert!(reference.contains_key(id), "{}: unknown message '{id}'", path.display());
                        used.insert(id.to_string());
                    }
                }
                // key names are looked up indirectly by `Key::localized_name`
                for (pos, _) in text.match_indices("\"key-") {
                    let rest = &text[pos + 1..];
                    used.insert(rest[..rest.find('"').unwrap()].to_string());
                }
            }
        }
        let unused: Vec<_> = reference.keys().filter(|k| !used.contains(*k)).collect();
        assert!(unused.is_empty(), "messages of en.ftl not used in the source: {unused:?}");
    }

    #[test]
    fn plurals_follow_the_language_rules() {
        let n = |count: u32| {
            let mut a = FluentArgs::new();
            a.set("count", count);
            a
        };
        assert_eq!(tr_in(Lang::En, "msg-combos-imported", Some(&n(1))).unwrap(), "1 combo was imported.");
        assert_eq!(tr_in(Lang::En, "msg-combos-imported", Some(&n(0))).unwrap(), "0 combos were imported.");
        assert_eq!(tr_in(Lang::It, "msg-combos-imported", Some(&n(3))).unwrap(), "3 combo importati.");
        // in French, 0 and 1 are singular
        assert_eq!(tr_in(Lang::Fr, "msg-combos-imported", Some(&n(0))).unwrap(), "0 combo a été importé.");
        assert_eq!(tr_in(Lang::Fr, "msg-combos-imported", Some(&n(2))).unwrap(), "2 combos ont été importés.");
        assert_eq!(tr_in(Lang::Es, "msg-combos-imported", Some(&n(1))).unwrap(), "Se ha importado 1 combo.");
    }

    #[test]
    fn no_isolation_marks_and_french_spacing() {
        let mut a = FluentArgs::new();
        a.set("name", "Work");
        let s = tr_in(Lang::Fr, "confirm-delete-group", Some(&a)).unwrap();
        assert_eq!(s, "Supprimer le groupe «\u{a0}Work\u{a0}»\u{a0}?");
        assert!(!s.contains('\u{2068}'));
    }

    #[test]
    fn locale_tags() {
        assert_eq!(Lang::from_locale("fr-CA"), Some(Lang::Fr));
        assert_eq!(Lang::from_locale("es_ES.UTF-8"), Some(Lang::Es));
        assert_eq!(Lang::from_locale("IT"), Some(Lang::It));
        assert_eq!(Lang::from_locale("de-DE"), None);
    }
}
