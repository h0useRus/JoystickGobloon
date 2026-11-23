//! Host functions plugins call through the prelude's `xml`, `json`, `ini`, `luadata` and
//! `version` tables.

use mlua::{Lua, LuaSerdeExt, Table, Value};

use crate::{luadata, version, xml};

/// Puts the helpers into the host table the prelude receives.
pub fn register(lua: &Lua, host: &Table) -> mlua::Result<()> {
    host.set(
        "xml_parse",
        lua.create_function(|lua, text: String| xml::to_lua(lua, &text))?,
    )?;
    host.set(
        "xml_write",
        lua.create_function(|_, doc: Table| xml::from_lua(&doc))?,
    )?;
    host.set(
        "json_parse",
        lua.create_function(|lua, text: String| {
            let v: serde_json::Value = serde_json::from_str(&text).map_err(mlua::Error::runtime)?;
            lua.to_value_with(
                &v,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            )
        })?,
    )?;
    host.set(
        "json_write",
        lua.create_function(|lua, v: Value| {
            let v: serde_json::Value = lua.from_value(v)?;
            serde_json::to_string_pretty(&v).map_err(mlua::Error::runtime)
        })?,
    )?;
    host.set(
        "ini_parse",
        lua.create_function(|lua, text: String| {
            let t = lua.create_table()?;
            for (section, entries) in parse_ini(&text) {
                let s = lua.create_table()?;
                for (k, v) in entries {
                    s.set(k, v)?;
                }
                t.set(section, s)?;
            }
            Ok(t)
        })?,
    )?;
    host.set(
        "luadata_parse",
        lua.create_function(|lua, text: String| {
            let data = luadata::parse(&text).map_err(mlua::Error::runtime)?;
            luadata::to_lua(lua, &data)
        })?,
    )?;
    host.set(
        "luadata_write",
        lua.create_function(|_, v: Value| Ok(luadata::write(&luadata::from_lua(&v)?)))?,
    )?;
    host.set(
        "version_compare",
        lua.create_function(|_, (a, b): (String, String)| Ok(version::compare(&a, &b) as i8))?,
    )?;
    Ok(())
}

/// INI text: `[section]`, `key=value`, `;` and `#` comments; keys before any section go under "".
pub fn parse_ini(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out: Vec<(String, Vec<(String, String)>)> = vec![(String::new(), Vec::new())];
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push((name.trim().to_string(), Vec::new()));
        } else if let Some((k, v)) = line.split_once('=')
            && let Some((_, entries)) = out.last_mut()
        {
            entries.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    out.retain(|(name, entries)| !name.is_empty() || !entries.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_sections_and_comments() {
        let ini = parse_ini("\u{feff}top=1\n; note\n[a]\nk = v=w\n# x\n[b]\n");
        assert_eq!(
            ini,
            [
                ("".into(), vec![("top".into(), "1".into())]),
                ("a".into(), vec![("k".into(), "v=w".into())]),
                ("b".into(), vec![]),
            ]
        );
    }
}
