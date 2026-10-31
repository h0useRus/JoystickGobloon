//! Fluent-based localization. The app embeds its languages and passes them in; English is always
//! the fallback. Extra or overriding languages are loaded from a folder of `<code>.ftl` files.

use std::collections::BTreeMap;
use std::path::Path;

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub use fluent_bundle::{FluentArgs, FluentValue};

pub const DEFAULT_LANG: &str = "en";

type Bundle = FluentBundle<FluentResource>;

pub struct I18n {
    sources: BTreeMap<String, String>,
    fallback: Bundle,
    current: Option<Bundle>,
    current_code: String,
}

impl I18n {
    /// `embedded`: the app's built-in languages as `(code, Fluent source)`, English among them;
    /// then any `*.ftl` from `lang_dir` (overrides embedded ones).
    pub fn new(embedded: &[(&str, &str)], lang_dir: Option<&Path>) -> Self {
        let mut sources: BTreeMap<String, String> = embedded
            .iter()
            .map(|(c, s)| (c.to_string(), s.to_string()))
            .collect();
        if let Some(dir) = lang_dir
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            for path in entries.flatten().map(|e| e.path()) {
                if path.extension().is_some_and(|e| e == "ftl")
                    && let (Some(code), Ok(text)) = (
                        path.file_stem().and_then(|s| s.to_str()),
                        std::fs::read_to_string(&path),
                    )
                {
                    sources.insert(code.to_string(), text);
                }
            }
        }
        let english = sources.get(DEFAULT_LANG).map_or("", String::as_str);
        let fallback = build_bundle(DEFAULT_LANG, english);
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

/// Checks that every language in `sources` (`(code, Fluent source)`) has every key of English;
/// the error lists each missing key as `<code>: <key>`. For an app's test of its own languages.
pub fn check_complete(sources: &[(&str, &str)]) -> Result<(), String> {
    let Some((_, en)) = sources.iter().find(|(code, _)| *code == DEFAULT_LANG) else {
        return Err(format!("no `{DEFAULT_LANG}` source"));
    };
    let en = keys(en);
    let missing: Vec<String> = sources
        .iter()
        .filter(|(code, _)| *code != DEFAULT_LANG)
        .flat_map(|(code, src)| {
            let other = keys(src);
            en.iter()
                .filter(move |k| !other.contains(k))
                .map(move |k| format!("{code}: {k}"))
        })
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("missing keys:\n{}", missing.join("\n")))
    }
}

/// Message ids defined in a Fluent source.
fn keys(src: &str) -> Vec<&str> {
    src.lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim()))
        .filter(|k| !k.is_empty() && !k.starts_with('#'))
        .collect()
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

    const EN: &str = "language-name = English\ngreeting = Hello\nbye = Bye\n";
    const RU: &str = "language-name = Русский\ngreeting = Привет\n";

    #[test]
    fn falls_back_to_english() {
        let mut i = I18n::new(&[("en", EN), ("ru", RU)], None);
        i.set_language("ru");
        assert_eq!(i.language(), "ru");
        assert_eq!(i.tr("greeting"), "Привет");
        assert_eq!(i.tr("bye"), "Bye", "missing in ru");
        i.set_language("xx");
        assert_eq!(i.language(), "en");
        assert_eq!(i.tr("greeting"), "Hello");
        assert_eq!(i.tr("no-such-key"), "no-such-key");
        assert_eq!(
            i.languages(),
            [
                ("en".to_string(), "English".to_string()),
                ("ru".to_string(), "Русский".to_string())
            ]
        );
    }

    #[test]
    fn check_complete_names_missing_keys() {
        assert_eq!(check_complete(&[("en", EN), ("ru", EN)]), Ok(()));
        let err = check_complete(&[("en", EN), ("ru", RU)]).unwrap_err();
        assert!(err.contains("ru: bye"), "{err}");
        assert!(check_complete(&[("ru", RU)]).is_err(), "no English");
    }
}
