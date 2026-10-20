//! Chinese and Korean glyphs from the system's fonts, found through fontconfig (`fc-list`).
//! Noto Sans CJK / Source Han Sans cover both languages in one file, which is then loaded once.

use std::path::PathBuf;
use std::process::Command;

/// Font file names preferred, in order, among those fontconfig lists for a language.
const PREFERRED: [&str; 4] = ["NotoSansCJK", "SourceHanSans", "NotoSans", "wqy"];

/// Font files that have glyphs for `lang` (e.g. `zh-cn`).
fn files_for(lang: &str) -> Vec<String> {
    let output = Command::new("fc-list")
        .args(["-f", "%{file}\\n", &format!(":lang={lang}")])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect(),
        Ok(_) => Vec::new(),
        Err(e) => {
            log::warn!("fc-list: {e}");
            Vec::new()
        }
    }
}

/// The best of `files`: a preferred family, regular weight first; then any.
fn pick(mut files: Vec<String>) -> Option<String> {
    files.sort();
    let regular = |f: &&String| !f.contains("Bold") && !f.contains("Light") && !f.contains("Thin");
    PREFERRED
        .iter()
        .find_map(|p| {
            let named = files.iter().filter(|f| f.contains(p));
            named
                .clone()
                .find(regular)
                .or_else(|| files.iter().find(|f| f.contains(p)))
        })
        .or_else(|| files.iter().find(regular))
        .or(files.first())
        .cloned()
}

pub(super) fn cjk() -> Vec<(&'static str, PathBuf)> {
    let zh = pick(files_for("zh-cn"));
    let ko = pick(files_for("ko"));
    let mut fonts = Vec::new();
    if let Some(f) = &zh {
        fonts.push(("cjk-zh", PathBuf::from(f)));
    }
    if let Some(f) = ko.filter(|k| Some(k) != zh.as_ref()) {
        fonts.push(("cjk-ko", PathBuf::from(f)));
    }
    if fonts.is_empty() {
        log::warn!("no Chinese or Korean font found (install Noto Sans CJK)");
    }
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_regular_noto_cjk() {
        let files = vec![
            "/usr/share/fonts/wqy/wqy-zenhei.ttc".to_string(),
            "/usr/share/fonts/noto/NotoSansCJK-Bold.ttc".to_string(),
            "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc".to_string(),
        ];
        assert_eq!(
            pick(files).as_deref(),
            Some("/usr/share/fonts/noto/NotoSansCJK-Regular.ttc")
        );
        let other = vec!["/f/Other-Bold.ttf".to_string(), "/f/Other.ttf".to_string()];
        assert_eq!(pick(other).as_deref(), Some("/f/Other.ttf"));
        assert_eq!(pick(Vec::new()), None);
    }
}
