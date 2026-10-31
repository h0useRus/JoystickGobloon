//! XML text <-> [`Element`] tree. Whitespace-only text between child elements
//! is dropped; all other text (including leading/trailing spaces) is kept.

use gob_core::{Element, Node};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::Error;

pub fn parse(text: &str) -> Result<Element, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = Reader::from_str(text);
    let xml = |e: &dyn std::fmt::Display| Error::Xml(e.to_string());
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| Error::Xml(format!("{e} at byte {}", reader.buffer_position())))?;
        match event {
            Event::Start(e) => stack.push(start(&e).map_err(|e| xml(&e))?),
            Event::Empty(e) => attach(&mut stack, &mut root, start(&e).map_err(|e| xml(&e))?),
            Event::End(_) => {
                let mut el = stack.pop().ok_or_else(|| xml(&"unexpected closing tag"))?;
                if el.elements().next().is_some() {
                    el.children
                        .retain(|n| !matches!(n, Node::Text(t) if t.trim().is_empty()));
                }
                attach(&mut stack, &mut root, el);
            }
            Event::Text(t) => push_text(&mut stack, &t.xml10_content()),
            Event::CData(c) => push_text(&mut stack, &c.xml10_content()),
            // Entities come apart from the text around them: `&amp;` or `&#38;`.
            Event::GeneralRef(r) => {
                let mut buf = [0; 4];
                let text = match r.resolve_char_ref().map_err(|e| xml(&e))? {
                    Some(c) => &*c.encode_utf8(&mut buf),
                    None => quick_xml::escape::resolve_predefined_entity(&r)
                        .ok_or_else(|| xml(&format!("unknown entity &{};", &*r)))?,
                };
                push_text(&mut stack, text);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| Error::Xml("document has no root element".into()))
}

fn start(e: &BytesStart) -> Result<Element, quick_xml::Error> {
    let mut el = Element::new(e.name().as_ref());
    for attr in e.attributes() {
        let attr = attr.map_err(quick_xml::Error::from)?;
        el.attrs.push((
            attr.key.as_ref().to_owned(),
            quick_xml::escape::unescape(&attr.value)?.into_owned(),
        ));
    }
    Ok(el)
}

/// Text, CDATA and entities next to each other make one text node.
fn push_text(stack: &mut [Element], text: &str) {
    let Some(top) = stack.last_mut() else {
        return;
    };
    match top.children.last_mut() {
        Some(Node::Text(t)) => t.push_str(text),
        _ => top.children.push(Node::Text(text.to_owned())),
    }
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, el: Element) {
    match stack.last_mut() {
        Some(parent) => parent.push(el),
        None => *root = Some(el),
    }
}

/// Serializes like Gremlin (Python minidom): 4-space indent, leaf text inline.
pub fn write(root: &Element) -> String {
    let mut out = String::from("<?xml version=\"1.0\" ?>\n");
    write_element(&mut out, root, 0);
    out
}

fn write_element(out: &mut String, el: &Element, depth: usize) {
    let indent = "    ".repeat(depth);
    out.push_str(&indent);
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attrs {
        out.push_str(&format!(
            " {k}=\"{}\"",
            quick_xml::escape::escape(v.as_str())
        ));
    }
    let has_elements = el.elements().next().is_some();
    let text = el.text();
    if !has_elements && text.is_empty() {
        out.push_str("/>\n");
    } else if !has_elements {
        out.push('>');
        out.push_str(&quick_xml::escape::partial_escape(text.as_str()));
        out.push_str(&format!("</{}>\n", el.name));
    } else {
        out.push_str(">\n");
        for child in el.elements() {
            write_element(out, child, depth + 1);
        }
        out.push_str(&format!("{indent}</{}>\n", el.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_and_cdata_join_the_text() {
        let el = parse(r#"<a x="1 &amp; 2">b &lt; c &#65;<![CDATA[<d>]]></a>"#).unwrap();
        assert_eq!(el.attr("x"), Some("1 & 2"));
        assert_eq!(el.text(), "b < c A<d>");
        assert_eq!(el.children.len(), 1);
        assert!(parse("<a>&nope;</a>").is_err());
    }
}
