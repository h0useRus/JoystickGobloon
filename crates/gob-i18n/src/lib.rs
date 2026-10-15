//! Fluent-based localization. English is embedded and always used as fallback.
//! Extra or overriding languages are loaded from `<exe dir>/lang/<code>.ftl`.

use std::collections::BTreeMap;
use std::path::Path;

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub use fluent_bundle::{FluentArgs, FluentValue};

pub const DEFAULT_LANG: &str = "en";

const EMBEDDED: &[(&str, &str)] = &[
    ("en", include_str!("../../../lang/en.ftl")),
    ("ru", include_str!("../../../lang/ru.ftl")),
];

type Bundle = FluentBundle<FluentResource>;

pub struct I18n {
    sources: BTreeMap<String, String>,
    fallback: Bundle,
    current: Option<Bundle>,
    current_code: String,
}

impl I18n {
    /// Loads embedded languages, then any `*.ftl` from `lang_dir` (overrides embedded ones).
    pub fn new(lang_dir: Option<&Path>) -> Self {
        let mut sources: BTreeMap<String, String> = EMBEDDED
            .iter()
            .map(|(c, s)| (c.to_string(), s.to_string()))
            .collect();
        if let Some(dir) = lang_dir {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for path in entries.flatten().map(|e| e.path()) {
                    if path.extension().is_some_and(|e| e == "ftl") {
                        if let (Some(code), Ok(text)) = (
                            path.file_stem().and_then(|s| s.to_str()),
                            std::fs::read_to_string(&path),
                        ) {
                            sources.insert(code.to_string(), text);
                        }
                    }
                }
            }
        }
        let fallback = build_bundle(DEFAULT_LANG, &sources[DEFAULT_LANG]);
        Self {
            sources,
            fallback,
            current: None,
            current_code: DEFAULT_LANG.into(),
        }
    }

    /// Switches language; unknown codes fall back to English.
    pub fn set_language(&mut self, code: &str) {
        match self.sources.get(code) {
            Some(src) if code != DEFAULT_LANG => {
                self.current = Some(build_bundle(code, src));
                self.current_code = code.into();
            }
            _ => {
                self.current = None;
                self.current_code = DEFAULT_LANG.into();
            }
        }
    }

    pub fn language(&self) -> &str {
        &self.current_code
    }

    /// `(code, display name)` of every available language.
    pub fn languages(&self) -> Vec<(String, String)> {
        self.sources
            .iter()
            .map(|(code, src)| {
                let name = format_in(&build_bundle(code, src), "language-name", None)
                    .unwrap_or_else(|| code.clone());
                (code.clone(), name)
            })
            .collect()
    }

    pub fn tr(&self, key: &str) -> String {
        self.tr_args(key, None)
    }

    /// `tr` with named arguments: `i18n.trf("key", &[("count", 3.into())])`.
    pub fn trf(&self, key: &str, args: &[(&str, FluentValue<'static>)]) -> String {
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            fa.set(*k, v.clone());
        }
        self.tr_args(key, Some(&fa))
    }

    pub fn tr_args(&self, key: &str, args: Option<&FluentArgs>) -> String {
        self.current
            .as_ref()
            .and_then(|b| format_in(b, key, args))
            .or_else(|| format_in(&self.fallback, key, args))
            .unwrap_or_else(|| key.to_string())
    }
}

fn build_bundle(code: &str, src: &str) -> Bundle {
    let lang: LanguageIdentifier = code.parse().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![lang]);
    bundle.set_use_isolating(false);
    // Parse errors keep the valid entries; broken lines are simply skipped.
    let res = FluentResource::try_new(src.to_string()).unwrap_or_else(|(res, _)| res);
    let _ = bundle.add_resource(res);
    bundle
}

fn format_in(bundle: &Bundle, key: &str, args: Option<&FluentArgs>) -> Option<String> {
    let pattern = bundle.get_message(key)?.value()?;
    let mut errors = vec![];
    Some(
        bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(src: &str) -> Vec<&str> {
        src.lines()
            .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim()))
            .filter(|k| !k.is_empty() && !k.starts_with('#'))
            .collect()
    }

    #[test]
    fn all_embedded_languages_have_every_english_key() {
        let en = keys(EMBEDDED[0].1);
        for (code, src) in &EMBEDDED[1..] {
            let other = keys(src);
            for k in &en {
                assert!(other.contains(k), "{code}.ftl is missing key `{k}`");
            }
        }
    }

    #[test]
    fn falls_back_to_english() {
        let mut i = I18n::new(None);
        i.set_language("ru");
        assert_eq!(i.tr("nav-settings"), "Настройки");
        i.set_language("xx");
        assert_eq!(i.language(), "en");
        assert_eq!(i.tr("nav-settings"), "Settings");
        assert_eq!(i.tr("no-such-key"), "no-such-key");
    }
}
