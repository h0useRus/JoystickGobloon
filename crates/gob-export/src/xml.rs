//! XML for plugins without losing the file's formatting. `parse` gives Luau a tree of plain tables
//! (`{tag, attrs, children}`, text as strings, comments and the like as `{raw}`) and keeps the
//! original text; `write` copies every element the plugin left alone byte for byte from it and
//! only builds the changed ones anew, keeping their start tag when the attributes did not change.

use std::ops::Range;
use std::sync::Arc;

use mlua::{AnyUserData, Lua, Table, UserData, Value};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

#[derive(Debug)]
enum XNode {
    El(usize),
    Text(String),
    /// Comments, declarations, processing instructions, CDATA: kept as written.
    Raw(Range<usize>),
}

#[derive(Debug)]
struct XEl {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<XNode>,
    /// The whole element in the source.
    span: Range<usize>,
    /// Its start tag (the whole element when it is self-closing).
    start: Range<usize>,
    empty: bool,
}

/// A parsed document: the source and every element in it.
#[derive(Debug)]
pub struct Doc {
    src: String,
    els: Vec<XEl>,
    top: Vec<XNode>,
}

struct DocData(Arc<Doc>);
impl UserData for DocData {}

/// Elements nested deeper than this are refused, read or written: a deeper tree (or a table that
/// holds itself) would overflow the stack.
const MAX_DEPTH: usize = 256;

impl Doc {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut reader = Reader::from_str(text);
        let mut doc = Doc {
            src: text.to_string(),
            els: Vec::new(),
            top: Vec::new(),
        };
        // Open elements: index in `els`.
        let mut stack: Vec<usize> = Vec::new();
        loop {
            let from = reader.buffer_position() as usize;
            let event = reader
                .read_event()
                .map_err(|e| format!("{e} at byte {}", reader.buffer_position()))?;
            let to = reader.buffer_position() as usize;
            let node = match event {
                Event::Start(e) | Event::Empty(e) => {
                    let empty = text.as_bytes()[..to].ends_with(b"/>");
                    let id = doc.els.len();
                    doc.els.push(XEl {
                        tag: e.name().as_ref().to_string(),
                        attrs: attrs(&e)?,
                        children: Vec::new(),
                        span: from..to,
                        start: from..to,
                        empty,
                    });
                    if !empty {
                        if stack.len() >= MAX_DEPTH {
                            return Err(format!("elements nested deeper than {MAX_DEPTH}"));
                        }
                        doc.push(&stack, XNode::El(id));
                        stack.push(id);
                        continue;
                    }
                    XNode::El(id)
                }
                Event::End(_) => {
                    let id = stack.pop().ok_or("unexpected closing tag")?;
                    doc.els[id].span.end = to;
                    continue;
                }
                // As written (line ends too); entities come as their own events.
                Event::Text(_) => XNode::Text(text[from..to].to_string()),
                Event::GeneralRef(r) => {
                    let mut buf = [0; 4];
                    let s = match r.resolve_char_ref().map_err(|e| e.to_string())? {
                        Some(c) => c.encode_utf8(&mut buf).to_string(),
                        None => quick_xml::escape::resolve_predefined_entity(&r)
                            .ok_or_else(|| format!("unknown entity &{};", &*r))?
                            .to_string(),
                    };
                    XNode::Text(s)
                }
                Event::Eof => break,
                _ => XNode::Raw(from..to),
            };
            doc.push(&stack, node);
        }
        if let Some(&open) = stack.last() {
            return Err(format!("<{}> is not closed", doc.els[open].tag));
        }
        Ok(doc)
    }

    /// Adds a node to the open element (or the top level); text next to text joins it.
    fn push(&mut self, stack: &[usize], node: XNode) {
        let list = match stack.last() {
            Some(&id) => &mut self.els[id].children,
            None => &mut self.top,
        };
        if let (XNode::Text(new), Some(XNode::Text(last))) = (&node, list.last_mut()) {
            last.push_str(new);
            return;
        }
        list.push(node);
    }
}

fn attrs(e: &BytesStart) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for a in e.attributes().with_checks(false) {
        let a = a.map_err(|e| e.to_string())?;
        let value = quick_xml::escape::unescape(&a.value)
            .map_err(|e| e.to_string())?
            .into_owned();
        out.push((a.key.as_ref().to_string(), value));
    }
    Ok(out)
}

// ---- Lua side -------------------------------------------------------------------------------

/// `xml.parse(text)`: `{children = {...}, _doc = <source>}`.
pub fn to_lua(lua: &Lua, text: &str) -> mlua::Result<Table> {
    let doc = Arc::new(Doc::parse(text).map_err(mlua::Error::runtime)?);
    let t = lua.create_table()?;
    t.set("children", nodes_to_lua(lua, &doc, &doc.top)?)?;
    t.set("_doc", lua.create_userdata(DocData(doc))?)?;
    Ok(t)
}

fn nodes_to_lua(lua: &Lua, doc: &Doc, nodes: &[XNode]) -> mlua::Result<Table> {
    let list = lua.create_table_with_capacity(nodes.len(), 0)?;
    for node in nodes {
        match node {
            XNode::Text(s) => list.push(s.as_str())?,
            XNode::Raw(r) => {
                let t = lua.create_table()?;
                t.set("raw", &doc.src[r.clone()])?;
                list.push(t)?;
            }
            XNode::El(id) => {
                let el = &doc.els[*id];
                let t = lua.create_table()?;
                t.set("tag", el.tag.as_str())?;
                let attrs = lua.create_table()?;
                for (k, v) in &el.attrs {
                    attrs.set(k.as_str(), v.as_str())?;
                }
                t.set("attrs", attrs)?;
                t.set("children", nodes_to_lua(lua, doc, &el.children)?)?;
                t.set("_id", *id)?;
                list.push(t)?;
            }
        }
    }
    Ok(list)
}

/// `xml.write(doc)`: the document's text; untouched elements as they were written.
pub fn from_lua(doc: &Table) -> mlua::Result<String> {
    let original = match doc.get::<Option<AnyUserData>>("_doc")? {
        Some(ud) => Some(ud.borrow::<DocData>()?.0.clone()),
        None => None,
    };
    let mut out = String::new();
    let children: Table = doc.get("children")?;
    write_nodes(&mut out, original.as_deref(), &children, 0)?;
    Ok(out)
}

fn write_nodes(
    out: &mut String,
    doc: Option<&Doc>,
    nodes: &Table,
    depth: usize,
) -> mlua::Result<()> {
    if depth > MAX_DEPTH {
        return Err(mlua::Error::runtime(format!(
            "XML elements nested deeper than {MAX_DEPTH} (a table that holds itself?)"
        )));
    }
    for node in nodes.sequence_values::<Value>() {
        match node? {
            Value::String(s) => escape_text(out, &s.to_str()?),
            Value::Table(t) => write_table(out, doc, &t, depth)?,
            Value::Integer(i) => out.push_str(&i.to_string()),
            Value::Number(n) => out.push_str(&n.to_string()),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "an XML node is a string or a table, not {}",
                    other.type_name()
                )));
            }
        }
    }
    Ok(())
}

/// Text as XML: `&`, `<` and `>` escaped, everything else (line ends too) as it is.
fn escape_text(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn original<'a>(doc: Option<&'a Doc>, t: &Table) -> mlua::Result<Option<&'a XEl>> {
    let (Some(doc), Some(id)) = (doc, t.get::<Option<usize>>("_id")?) else {
        return Ok(None);
    };
    Ok(doc.els.get(id))
}

fn write_table(out: &mut String, doc: Option<&Doc>, t: &Table, depth: usize) -> mlua::Result<()> {
    if let Some(raw) = t.get::<Option<String>>("raw")? {
        out.push_str(&raw);
        return Ok(());
    }
    let tag: String = t
        .get::<Option<String>>("tag")?
        .ok_or_else(|| mlua::Error::runtime("an XML element needs a tag"))?;
    let orig = original(doc, t)?;
    if let (Some(doc), Some(orig)) = (doc, orig)
        && same_element(doc, orig, t)?
    {
        out.push_str(&doc.src[orig.span.clone()]);
        return Ok(());
    }
    let attrs = ordered_attrs(t, orig)?;
    let children: Option<Table> = t.get("children")?;
    let has_children = children.as_ref().is_some_and(|c| c.raw_len() > 0);
    match (doc, orig) {
        (Some(doc), Some(orig)) if orig.tag == tag && same_attrs(orig, &attrs) => {
            let start = &doc.src[orig.start.clone()];
            if !orig.empty {
                out.push_str(start);
            } else if has_children {
                out.push_str(start.trim_end_matches("/>").trim_end());
                out.push('>');
            } else {
                out.push_str(start);
                return Ok(());
            }
        }
        _ => {
            out.push('<');
            out.push_str(&tag);
            for (k, v) in &attrs {
                out.push_str(&format!(
                    " {k}=\"{}\"",
                    quick_xml::escape::escape(v.as_str())
                ));
            }
            if !has_children {
                out.push_str("/>");
                return Ok(());
            }
            out.push('>');
        }
    }
    if let Some(children) = children {
        write_nodes(out, doc, &children, depth + 1)?;
    }
    out.push_str(&format!("</{tag}>"));
    Ok(())
}

/// The attributes in the original order, then `order` (new elements), then the rest by name.
fn ordered_attrs(t: &Table, orig: Option<&XEl>) -> mlua::Result<Vec<(String, String)>> {
    let mut all: Vec<(String, String)> = Vec::new();
    if let Some(attrs) = t.get::<Option<Table>>("attrs")? {
        for pair in attrs.pairs::<String, Value>() {
            let (k, v) = pair?;
            let v = match v {
                Value::String(s) => s.to_str()?.to_string(),
                Value::Integer(i) => i.to_string(),
                Value::Number(n) => n.to_string(),
                Value::Boolean(b) => b.to_string(),
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "attribute '{k}' is a {}",
                        other.type_name()
                    )));
                }
            };
            all.push((k, v));
        }
    }
    let mut rank: Vec<String> = orig
        .map(|o| o.attrs.iter().map(|(k, _)| k.clone()).collect())
        .unwrap_or_default();
    if let Some(order) = t.get::<Option<Table>>("order")? {
        for k in order.sequence_values::<String>() {
            rank.push(k?);
        }
    }
    all.sort_by(|(a, _), (b, _)| {
        let pos = |k: &str| rank.iter().position(|r| r == k).unwrap_or(usize::MAX);
        pos(a).cmp(&pos(b)).then_with(|| a.cmp(b))
    });
    Ok(all)
}

fn same_attrs(orig: &XEl, attrs: &[(String, String)]) -> bool {
    orig.attrs.len() == attrs.len()
        && orig
            .attrs
            .iter()
            .all(|(k, v)| attrs.iter().any(|(k2, v2)| k == k2 && v == v2))
}

fn same_element(doc: &Doc, orig: &XEl, t: &Table) -> mlua::Result<bool> {
    if t.get::<Option<String>>("tag")?.as_deref() != Some(orig.tag.as_str())
        || !same_attrs(orig, &ordered_attrs(t, Some(orig))?)
    {
        return Ok(false);
    }
    let children: Option<Table> = t.get("children")?;
    let len = children.as_ref().map_or(0, |c| c.raw_len());
    if len != orig.children.len() {
        return Ok(false);
    }
    let Some(children) = children else {
        return Ok(true);
    };
    for (i, node) in orig.children.iter().enumerate() {
        let value: Value = children.raw_get(i + 1)?;
        let same = match (node, value) {
            (XNode::Text(a), Value::String(b)) => b.to_str()? == a.as_str(),
            (XNode::Raw(r), Value::Table(b)) => {
                b.get::<Option<String>>("raw")?.as_deref() == Some(&doc.src[r.clone()])
            }
            (XNode::El(id), Value::Table(b)) => {
                b.get::<Option<usize>>("_id")? == Some(*id) && same_element(doc, &doc.els[*id], &b)?
            }
            _ => false,
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_documents_and_tables_that_hold_themselves_fail_cleanly() {
        let n = 100_000;
        let text = format!("<a>{}{}</a>", "<b>".repeat(n), "</b>".repeat(n));
        assert!(Doc::parse(&text).is_err());
        let lua = Lua::new();
        let doc = to_lua(&lua, "<a/>").unwrap();
        lua.load("local d = ... local e = d.children[1] e.children = { e }")
            .call::<()>(doc.clone())
            .unwrap();
        assert!(from_lua(&doc).is_err());
    }

    fn round(text: &str, edit: &str) -> String {
        let lua = Lua::new();
        let doc = to_lua(&lua, text).unwrap();
        lua.load(edit).call::<()>(doc.clone()).unwrap();
        from_lua(&doc).unwrap()
    }

    const SAMPLE: &str = "<?xml version=\"1.0\"?>\n<!-- head -->\n<root a=\"1\"  b='2'>\n  <x  name=\"v_a\">\n    <rebind input=\"js1_button3\" />\n  </x>\n  <y>t &amp; u</y>\n  <empty></empty>\n</root>\n";

    #[test]
    fn untouched_documents_come_back_byte_for_byte() {
        assert_eq!(round(SAMPLE, "local doc = ..."), SAMPLE);
    }

    #[test]
    fn only_the_changed_element_is_written_anew() {
        let out = round(
            SAMPLE,
            r#"local doc = ...
            local root = doc.children[5]
            local x = root.children[2]
            x.children[2].attrs.input = "js2_button5""#,
        );
        assert_eq!(
            out,
            SAMPLE.replace(
                "<rebind input=\"js1_button3\" />",
                "<rebind input=\"js2_button5\"/>"
            )
        );
    }

    #[test]
    fn new_children_keep_the_parent_start_tag() {
        let out = round(
            SAMPLE,
            r#"local doc = ...
            local root = doc.children[5]
            local empty = root.children[6]
            table.insert(empty.children, {tag = "n", attrs = {z = "1", a = "2"}, order = {"z"}})
            table.insert(root.children, {tag = "added", attrs = {k = "a<b"}})"#,
        );
        assert!(out.contains("<empty><n z=\"1\" a=\"2\"/></empty>"), "{out}");
        assert!(out.contains("<root a=\"1\"  b='2'>"), "{out}");
        assert!(out.contains("<added k=\"a&lt;b\"/></root>"), "{out}");
        assert!(out.contains("<y>t &amp; u</y>"), "{out}");
    }

    #[test]
    fn self_closing_elements_open_up_for_children() {
        let out = round(
            "<a><b x=\"1\" /></a>",
            r#"local doc = ...
            table.insert(doc.children[1].children[1].children, "text")"#,
        );
        assert_eq!(out, "<a><b x=\"1\">text</b></a>");
    }

    #[test]
    fn broken_xml_is_an_error() {
        assert!(Doc::parse("<a><b></a>").is_err());
        assert!(Doc::parse("<a>").is_err());
    }

    #[test]
    fn line_ends_survive_changed_elements() {
        let out = round(
            "<a>\r\n <b x=\"1\"/>\r\n</a>\r\n",
            r#"local doc = ...
            doc.children[1].children[2].attrs.x = "2""#,
        );
        assert_eq!(out, "<a>\r\n <b x=\"2\"/>\r\n</a>\r\n");
    }
}
