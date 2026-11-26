//! Changes to a `device.gob` as text, so the author's comments, order and spacing stay: a
//! control's block is replaced, added or taken out, everything else is kept as it was.

use crate::plugin::{Control, Look, MAIN_VIEW, Side};

/// A new plugin's `device.gob`: the device it fits (when one was connected) and one view.
pub struct NewDevice<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub brand: &'a str,
    /// Vendor id of the connected device it is for; `None` = no `[[match]]` yet.
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    /// A part of the connected device's product name.
    pub product: Option<&'a str>,
    /// The picture's path in the plugin folder.
    pub image: &'a str,
}

pub fn new_manifest(d: &NewDevice) -> String {
    let mut out = format!(
        "id = {}\nname = {}\nbrand = {}\nversion = \"1\"\n",
        quote(d.id),
        quote(d.name),
        quote(d.brand),
    );
    if let Some(vid) = d.vid {
        out.push_str(&format!("\n[[match]]\nvid = 0x{vid:04X}\n"));
        if let Some(pid) = d.pid {
            out.push_str(&format!("pid = 0x{pid:04X}\n"));
        }
        if let Some(product) = d.product {
            out.push_str(&format!("name = {}\n", quote(product)));
        }
    }
    out.push_str(&format!(
        "\n[[view]]\nid = \"{MAIN_VIEW}\"\nname = {}\nimage = {}\n",
        quote(d.name),
        quote(d.image)
    ));
    out
}

/// A control's `[[control]]` block, ending with a newline.
pub fn control_text(c: &Control) -> String {
    let mut out = format!(
        "[[control]]\nid = {}\nname = {}\nkind = \"{}\"\n",
        quote(&c.id),
        quote(&c.name),
        kind_name(c)
    );
    if let Some(parts) = &c.parts {
        let list: Vec<String> = parts.iter().map(|p| quote(p)).collect();
        out.push_str(&format!("parts = [{}]\n", list.join(", ")));
    }
    if c.view != MAIN_VIEW {
        out.push_str(&format!("view = {}\n", quote(&c.view)));
    }
    out.push_str(&format!(
        "at = [{}, {}]\n",
        c.at[0].round(),
        c.at[1].round()
    ));
    if let Some([x, y]) = c.glyph {
        out.push_str(&format!("glyph = [{}, {}]\n", x.round(), y.round()));
    }
    if c.side == Side::Right {
        out.push_str("side = \"right\"\n");
    }
    if let Some(group) = &c.group {
        out.push_str(&format!("group = {}\n", quote(group)));
    }
    if c.look == Some(Look::Stick) {
        out.push_str("look = \"stick\"\n");
    }
    out
}

/// `text` with control `id`'s block replaced by `control` (which may have a new id), or
/// `control` added at the end when there is no such block.
pub fn set_control(text: &str, id: &str, control: &Control) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    match block(&lines, id) {
        Some((start, end)) => {
            let mut out: String = lines[..start].concat();
            out.push_str(&control_text(control));
            out.push_str(&lines[end..].concat());
            out
        }
        None => {
            let mut out = text.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&control_text(control));
            out
        }
    }
}

/// `text` without control `id`'s block (and the blank line after it).
pub fn remove_control(text: &str, id: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let Some((start, mut end)) = block(&lines, id) else {
        return text.to_string();
    };
    while end < lines.len() && lines[end].trim().is_empty() {
        end += 1;
    }
    let mut out: String = lines[..start].concat();
    out.push_str(&lines[end..].concat());
    out
}

/// Lines `start..end` of control `id`'s block: from its `[[control]]` line to the next table,
/// without the blank and comment lines before that table (they belong to it).
fn block(lines: &[&str], id: &str) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() != "[[control]]" {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + 1;
        while end < lines.len() && !lines[end].trim_start().starts_with('[') {
            end += 1;
        }
        let has_id = lines[start + 1..end].iter().any(|l| line_id(l) == Some(id));
        if has_id {
            while end > start + 1 {
                let last = lines[end - 1].trim();
                if last.is_empty() || last.starts_with('#') {
                    end -= 1;
                } else {
                    break;
                }
            }
            return Some((start, end));
        }
        i = end;
    }
    None
}

fn line_id(line: &str) -> Option<&str> {
    let rest = line
        .trim()
        .strip_prefix("id")?
        .trim_start()
        .strip_prefix('=')?;
    Some(rest.trim().trim_matches('"'))
}

fn kind_name(c: &Control) -> String {
    toml::Value::try_from(c.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// An id made of `text`: lowercase letters, digits and dashes (`Red button` -> `red-button`).
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Kind, Manifest};

    const TEXT: &str = r#"id = "x"
name = "X"
brand = "B"

[[match]]
vid = 0x0001
name = "X"

# The trigger, two stages.
[[control]]
id = "trigger"
name = "Trigger"
kind = "trigger"
at = [1, 2]

# A hat.
[[control]]
id = "hat"
name = "Hat"
kind = "hat"
at = [3, 4]
"#;

    fn control(id: &str) -> Control {
        Control {
            id: id.into(),
            name: "Fire \"A\"".into(),
            kind: Kind::Button,
            parts: None,
            view: MAIN_VIEW.into(),
            at: [10.4, 20.6],
            glyph: None,
            side: Side::Right,
            group: None,
            look: None,
        }
    }

    #[test]
    fn replaces_keeping_comments() {
        let out = set_control(TEXT, "trigger", &control("fire"));
        assert!(out.contains("# The trigger, two stages.\n[[control]]\nid = \"fire\""));
        assert!(out.contains("at = [10, 21]\nside = \"right\"\n\n# A hat."));
        let m: Manifest = toml::from_str(&out).unwrap();
        assert_eq!(m.controls[0].name, "Fire \"A\"");
        assert_eq!(m.controls[1].id, "hat");
    }

    #[test]
    fn adds_and_removes() {
        let out = set_control(TEXT, "new", &control("new"));
        let m: Manifest = toml::from_str(&out).unwrap();
        assert_eq!(m.controls.len(), 3);
        let out = remove_control(&out, "hat");
        let m: Manifest = toml::from_str(&out).unwrap();
        let ids: Vec<&str> = m.controls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["trigger", "new"]);
        assert!(!out.contains("kind = \"hat\""));
    }

    #[test]
    fn new_device() {
        let text = new_manifest(&NewDevice {
            id: "brand-stick",
            name: "Stick",
            brand: "Brand",
            vid: Some(0x231D),
            pid: None,
            product: Some("Stick L"),
            image: "views/main.png",
        });
        let m: Manifest = toml::from_str(&text).unwrap();
        assert_eq!(m.matches[0].vid, 0x231D);
        assert_eq!(m.views[0].image, "views/main.png");
        assert_eq!(slug("Gladiator NXT EVO (left)"), "gladiator-nxt-evo-left");
    }
}
