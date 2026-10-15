//! Minimal owned XML tree used to keep unknown profile content verbatim.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn with_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        let mut e = Self::new(name);
        e.children.push(Node::Text(text.into()));
        e
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push((key.into(), value.into()));
        self
    }

    pub fn push(&mut self, child: Element) {
        self.children.push(Node::Element(child));
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }

    /// Concatenated text content of direct text children.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) => Some(t.as_str()),
                Node::Element(_) => None,
            })
            .collect()
    }

    pub fn child_text(&self, name: &str) -> Option<String> {
        self.child(name).map(Element::text)
    }

    /// Replaces text nodes equal to `from` (ignoring ASCII case) anywhere in the tree.
    pub fn replace_text(&mut self, from: &str, to: &str) -> usize {
        let mut count = 0;
        for child in &mut self.children {
            match child {
                Node::Text(t) if t.eq_ignore_ascii_case(from) => {
                    *t = to.to_string();
                    count += 1;
                }
                Node::Element(e) => count += e.replace_text(from, to),
                Node::Text(_) => {}
            }
        }
        count
    }
}
