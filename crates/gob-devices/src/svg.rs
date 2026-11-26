//! The views' SVG pictures: one style for every device, colored by the program's theme.
//!
//! Pictures draw with five classes and no colors of their own; [`themed`] puts a style sheet
//! with the theme's colors in place of the picture's `<style>`:
//! `body` (filled shape), `shade` (darker filled shape), `line` (outline only),
//! `detail` (thin inner lines), `accent` (a colored control, e.g. a red button).

/// The theme's colors for a picture, as CSS colors (`#rrggbb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub outline: String,
    pub body: String,
    pub shade: String,
    pub detail: String,
    pub accent: String,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            outline: "#8a8f98".into(),
            body: "#24272c".into(),
            shade: "#1a1c20".into(),
            detail: "#4a4f57".into(),
            accent: "#c8402f".into(),
        }
    }

    pub fn light() -> Self {
        Self {
            outline: "#5b6068".into(),
            body: "#eceef1".into(),
            shade: "#d6d9de".into(),
            detail: "#a9aeb6".into(),
            accent: "#d24a38".into(),
        }
    }

    fn css(&self) -> String {
        let Self {
            outline,
            body,
            shade,
            detail,
            accent,
        } = self;
        format!(
            ".body{{fill:{body};stroke:{outline}}}.shade{{fill:{shade};stroke:{outline}}}\
             .line{{fill:none;stroke:{outline}}}.detail{{fill:none;stroke:{detail}}}\
             .accent{{fill:{accent};stroke:{outline}}}"
        )
    }
}

/// `svg` with its `<style>` replaced by the palette's (added after the root tag without one).
pub fn themed(svg: &str, palette: &Palette) -> String {
    let style = format!("<style>{}</style>", palette.css());
    if let (Some(start), Some(end)) = (svg.find("<style"), svg.find("</style>"))
        && start < end
    {
        return format!("{}{style}{}", &svg[..start], &svg[end + "</style>".len()..]);
    }
    match svg
        .find("<svg")
        .and_then(|s| svg[s..].find('>').map(|e| s + e + 1))
    {
        Some(at) => format!("{}{style}{}", &svg[..at], &svg[at..]),
        None => svg.to_string(),
    }
}

/// The picture's `viewBox`: x, y, width, height.
pub fn view_box(svg: &str) -> Option<[f32; 4]> {
    let start = svg.find("viewBox=\"")? + "viewBox=\"".len();
    let end = start + svg[start..].find('"')?;
    let numbers: Vec<f32> = svg[start..end]
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    numbers.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_style() {
        let svg = r#"<svg viewBox="0 0 1000 800"><style>.body{fill:red}</style><rect class="body"/></svg>"#;
        let out = themed(svg, &Palette::dark());
        assert!(!out.contains("fill:red"));
        assert!(out.contains(".body{fill:#24272c;stroke:#8a8f98}"));
        assert!(out.ends_with(r#"<rect class="body"/></svg>"#));
        assert_eq!(view_box(svg), Some([0.0, 0.0, 1000.0, 800.0]));
    }

    #[test]
    fn adds_style() {
        let out = themed(
            r#"<svg viewBox="0 0 10 10"><path/></svg>"#,
            &Palette::light(),
        );
        assert!(out.starts_with(r#"<svg viewBox="0 0 10 10"><style>"#));
    }
}
