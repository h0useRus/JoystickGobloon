//! User scripts (`<scripts>`): Lua scripts Joystick Gobloon runs, and Joystick Gremlin's Python
//! scripts, which are kept but not run. Each `<script>` is one instance: its source (code stored in
//! the profile or an external file), a name, and the values the user gave the variables the script
//! declares. Variables are written like Gremlin writes them (`<variable type="...">` with `name` and
//! per-type properties), so imported values read. Unknown children are kept as they are.

use crate::action::{InputType, Key};
use crate::profile::Profile;
use crate::xml::{Element, Node};

/// What a script is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptLanguage {
    Lua,
    /// Joystick Gremlin's user scripts: kept in the profile, not run.
    Python,
}

/// Where a script's code is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptSource {
    /// Stored in the profile.
    Code(String),
    /// An external file: absolute, or relative to the profile's folder.
    File(String),
}

/// A value the user gave a script variable.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    /// Index into the options the script declares.
    Selection(usize),
    Mode(String),
    Keyboard(Key),
    /// An input of a physical device.
    PhysicalInput {
        device: String,
        input_type: InputType,
        id: u32,
    },
    /// An output of a virtual stick (`vjoy` in Gremlin).
    Vjoy {
        device: u32,
        input_type: InputType,
        id: u32,
    },
    LogicalDevice {
        input_type: InputType,
        id: u32,
    },
}

impl ScriptValue {
    /// Gremlin's `type` attribute of the variable.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Selection(_) => "selection",
            Self::Mode(_) => "mode",
            Self::Keyboard(_) => "keyboard",
            Self::PhysicalInput { .. } => "physical-input",
            Self::Vjoy { .. } => "vjoy",
            Self::LogicalDevice { .. } => "logical-device",
        }
    }
}

/// One `<script>` of the profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptInstance {
    pub id: String,
    pub name: String,
    pub language: ScriptLanguage,
    pub source: ScriptSource,
    /// Values the user set, by variable name (unset ones are left out).
    pub variables: Vec<(String, ScriptValue)>,
}

fn property(el: &Element, name: &str) -> Option<String> {
    el.elements()
        .filter(|p| p.name == "property")
        .find(|p| p.child_text("name").is_some_and(|n| n.trim() == name))
        .and_then(|p| p.child_text("value"))
}

fn property_node(ty: &str, name: &str, value: impl Into<String>) -> Element {
    let mut p = Element::new("property").with_attr("type", ty);
    p.push(Element::with_text("name", name));
    p.push(Element::with_text("value", value));
    p
}

fn set_property(el: &mut Element, ty: &str, name: &str, value: impl Into<String>) {
    let node = property_node(ty, name, value);
    let at = el.children.iter().position(|n| {
        matches!(n, Node::Element(p) if p.name == "property"
            && p.child_text("name").is_some_and(|n| n.trim() == name))
    });
    match at {
        Some(i) => el.children[i] = Node::Element(node),
        None => el.push(node),
    }
}

fn input_type_name(t: InputType) -> &'static str {
    match t {
        InputType::Axis => "axis",
        InputType::Button => "button",
        InputType::Hat => "hat",
        InputType::Other => "button",
    }
}

fn read_value(var: &Element) -> Option<ScriptValue> {
    let prop = |n: &str| property(var, n).map(|v| v.trim().to_string());
    let num = |n: &str| prop(n).and_then(|v| v.parse::<f64>().ok());
    let int = |n: &str| prop(n).and_then(|v| v.parse::<i64>().ok());
    let flag = |n: &str| prop(n).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let input_type = || InputType::parse(&prop("input-type").unwrap_or_default());
    let id = || int("input-id").map(|i| i.max(0) as u32);
    Some(match var.attr("type")? {
        "bool" => ScriptValue::Bool(flag("value")),
        "int" => ScriptValue::Int(int("value")?),
        "float" => ScriptValue::Float(num("value")?),
        "string" => ScriptValue::String(property(var, "value").unwrap_or_default()),
        "selection" => ScriptValue::Selection(int("index")?.max(0) as usize),
        "mode" => ScriptValue::Mode(prop("value")?),
        "keyboard" => ScriptValue::Keyboard(Key {
            scan_code: int("scan-code")? as u16,
            extended: flag("is-extended"),
        }),
        "physical-input" => ScriptValue::PhysicalInput {
            device: prop("device-guid")?,
            input_type: input_type(),
            id: id()?,
        },
        "vjoy" => ScriptValue::Vjoy {
            device: int("vjoy-id")?.max(1) as u32,
            input_type: input_type(),
            id: id()?,
        },
        "logical-device" => ScriptValue::LogicalDevice {
            input_type: input_type(),
            id: id()?,
        },
        _ => return None,
    })
}

fn variable_node(name: &str, value: &ScriptValue) -> Element {
    let mut v = Element::new("variable").with_attr("type", value.type_name());
    v.push(property_node("string", "name", name));
    let b = |x: bool| if x { "True" } else { "False" };
    let props: Vec<Element> = match value {
        ScriptValue::Bool(x) => vec![property_node("bool", "value", b(*x))],
        ScriptValue::Int(x) => vec![property_node("int", "value", x.to_string())],
        ScriptValue::Float(x) => vec![property_node("float", "value", format!("{x:?}"))],
        ScriptValue::String(x) => vec![property_node("string", "value", x.as_str())],
        ScriptValue::Selection(i) => vec![property_node("int", "index", i.to_string())],
        ScriptValue::Mode(m) => vec![property_node("string", "value", m.as_str())],
        ScriptValue::Keyboard(k) => vec![
            property_node("int", "scan-code", k.scan_code.to_string()),
            property_node("bool", "is-extended", b(k.extended)),
        ],
        ScriptValue::PhysicalInput {
            device,
            input_type,
            id,
        } => vec![
            property_node("uuid", "device-guid", device.as_str()),
            property_node("input_type", "input-type", input_type_name(*input_type)),
            property_node("int", "input-id", id.to_string()),
        ],
        ScriptValue::Vjoy {
            device,
            input_type,
            id,
        } => vec![
            property_node("int", "vjoy-id", device.to_string()),
            property_node("input_type", "input-type", input_type_name(*input_type)),
            property_node("int", "input-id", id.to_string()),
        ],
        ScriptValue::LogicalDevice { input_type, id } => vec![
            property_node("input_type", "input-type", input_type_name(*input_type)),
            property_node("int", "input-id", id.to_string()),
        ],
    };
    for p in props {
        v.push(p);
    }
    v
}

fn variable_name(var: &Element) -> Option<String> {
    property(var, "name").map(|n| n.trim().to_string())
}

fn read_instance(el: &Element) -> ScriptInstance {
    // Older Gremlin profiles keep path and name as attributes.
    let path = property(el, "path")
        .or_else(|| el.attr("path").map(String::from))
        .map(|p| p.trim().to_string());
    let code = el.child("code").map(Element::text);
    let python = path
        .as_deref()
        .is_some_and(|p| p.to_ascii_lowercase().ends_with(".py"));
    let language = match el.attr("language") {
        Some(l) if l.eq_ignore_ascii_case("lua") => ScriptLanguage::Lua,
        Some(_) => ScriptLanguage::Python,
        None if python || code.is_none() && path.is_some() => ScriptLanguage::Python,
        None => ScriptLanguage::Lua,
    };
    let source = match (code, path) {
        (Some(code), _) => ScriptSource::Code(code),
        (None, Some(path)) => ScriptSource::File(path),
        (None, None) => ScriptSource::Code(String::new()),
    };
    ScriptInstance {
        id: el.attr("id").unwrap_or_default().to_string(),
        name: property(el, "name")
            .or_else(|| el.attr("name").map(String::from))
            .unwrap_or_default(),
        language,
        source,
        variables: el
            .elements()
            .filter(|v| v.name == "variable")
            .filter_map(|v| Some((variable_name(v)?, read_value(v)?)))
            .collect(),
    }
}

/// An external script's file: `path` as it is when absolute, else in `base` (the profile's folder).
pub fn script_path(path: &str, base: Option<&std::path::Path>) -> std::path::PathBuf {
    let p = std::path::Path::new(path.trim());
    match base {
        Some(base) if p.is_relative() => base.join(p),
        _ => p.to_path_buf(),
    }
}

impl Profile {
    /// The profile's scripts, in order.
    pub fn script_instances(&self) -> Vec<ScriptInstance> {
        self.scripts
            .elements()
            .filter(|e| e.name == "script")
            .map(read_instance)
            .collect()
    }

    pub fn script_instance(&self, id: &str) -> Option<ScriptInstance> {
        self.script_element(id).map(read_instance)
    }

    fn script_element(&self, id: &str) -> Option<&Element> {
        self.scripts
            .elements()
            .find(|e| e.name == "script" && e.attr("id") == Some(id))
    }

    fn script_element_mut(&mut self, id: &str) -> Option<&mut Element> {
        self.scripts.children.iter_mut().find_map(|n| match n {
            Node::Element(e) if e.name == "script" && e.attr("id") == Some(id) => Some(e),
            _ => None,
        })
    }

    /// Adds a Lua script named `name` with `source`; returns its id.
    pub fn add_script(&mut self, name: &str, source: ScriptSource) -> String {
        let id = crate::edit::new_uuid();
        let mut el = Element::new("script")
            .with_attr("id", id.as_str())
            .with_attr("language", "lua");
        el.push(property_node("string", "name", name));
        self.scripts.name = "scripts".into();
        self.scripts.push(el);
        self.add_section(crate::profile::Section::Scripts);
        self.set_script_source(&id, &source);
        id
    }

    pub fn remove_script(&mut self, id: &str) {
        self.scripts.children.retain(
            |n| !matches!(n, Node::Element(e) if e.name == "script" && e.attr("id") == Some(id)),
        );
    }

    /// Moves a script one place up (`up`) or down.
    pub fn move_script(&mut self, id: &str, up: bool) {
        let pos: Vec<usize> = self
            .scripts
            .children
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(n, Node::Element(e) if e.name == "script"))
            .map(|(i, _)| i)
            .collect();
        let Some(at) = pos.iter().position(
            |&i| matches!(&self.scripts.children[i], Node::Element(e) if e.attr("id") == Some(id)),
        ) else {
            return;
        };
        let other = if up { at.checked_sub(1) } else { Some(at + 1) };
        if let Some(&j) = other.and_then(|o| pos.get(o)) {
            self.scripts.children.swap(pos[at], j);
        }
    }

    pub fn set_script_name(&mut self, id: &str, name: &str) {
        if let Some(el) = self.script_element_mut(id) {
            set_property(el, "string", "name", name);
        }
    }

    /// Code stored in the profile, or an external file (the other one is removed).
    pub fn set_script_source(&mut self, id: &str, source: &ScriptSource) {
        let Some(el) = self.script_element_mut(id) else {
            return;
        };
        el.children.retain(|n| {
            !matches!(n, Node::Element(e) if e.name == "code"
                || e.name == "property" && e.child_text("name").is_some_and(|n| n.trim() == "path"))
        });
        match source {
            ScriptSource::Code(code) => el.push(Element::with_text("code", code.as_str())),
            ScriptSource::File(path) => {
                // After the name, like Gremlin writes it.
                let node = Node::Element(property_node("path", "path", path.as_str()));
                el.children.insert(0, node);
            }
        }
    }

    /// Sets (`Some`) or clears a variable's value.
    pub fn set_script_variable(&mut self, id: &str, name: &str, value: Option<&ScriptValue>) {
        let Some(el) = self.script_element_mut(id) else {
            return;
        };
        let at = el.children.iter().position(|n| {
            matches!(n, Node::Element(v) if v.name == "variable"
                && variable_name(v).as_deref() == Some(name))
        });
        match (at, value) {
            (Some(i), Some(v)) => el.children[i] = Node::Element(variable_node(name, v)),
            (Some(i), None) => {
                el.children.remove(i);
            }
            (None, Some(v)) => el.push(variable_node(name, v)),
            (None, None) => {}
        }
    }

    /// Puts the code of external Lua script files into the profile, for running it (the engine
    /// reads no files). Relative paths are taken from `base`, the profile's folder. Returns the
    /// scripts whose file could not be read, with the error.
    pub fn read_script_files(&mut self, base: Option<&std::path::Path>) -> Vec<(String, String)> {
        let mut failed = Vec::new();
        for s in self.script_instances() {
            let ScriptSource::File(path) = &s.source else {
                continue;
            };
            if s.language != ScriptLanguage::Lua {
                continue;
            }
            let path = script_path(path, base);
            match std::fs::read_to_string(&path) {
                Ok(code) => self.set_script_source(&s.id, &ScriptSource::Code(code)),
                Err(e) => failed.push((s.name.clone(), format!("{}: {e}", path.display()))),
            }
        }
        failed
    }

    /// Joystick Gremlin's Python scripts, kept but not run.
    pub fn has_python_scripts(&self) -> bool {
        self.script_instances()
            .iter()
            .any(|s| s.language == ScriptLanguage::Python)
    }

    /// Lua scripts are Joystick Gobloon's own (Gremlin's are Python).
    pub fn has_lua_scripts(&self) -> bool {
        self.script_instances()
            .iter()
            .any(|s| s.language == ScriptLanguage::Lua)
    }

    /// Script variables naming physical device `old` name `new` instead; returns how many.
    pub(crate) fn substitute_script_device(&mut self, old: &str, new: &str) -> usize {
        let mut count = 0;
        for script in self.scripts.children.iter_mut() {
            let Node::Element(script) = script else {
                continue;
            };
            for var in script.children.iter_mut() {
                let Node::Element(var) = var else {
                    continue;
                };
                if var.attr("type") != Some("physical-input") {
                    continue;
                }
                let guid = property(var, "device-guid");
                if guid.is_some_and(|g| g.trim().eq_ignore_ascii_case(old)) {
                    set_property(var, "uuid", "device-guid", new);
                    count += 1;
                }
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_and_variables_edit() {
        let mut p = Profile::default();
        let a = p.add_script("Gear", ScriptSource::Code("print(1)\n".into()));
        let b = p.add_script("Trim", ScriptSource::File("trim.lua".into()));
        p.set_script_variable(&a, "Speed", Some(&ScriptValue::Float(1.5)));
        p.set_script_variable(
            &a,
            "Stick",
            Some(&ScriptValue::PhysicalInput {
                device: "AAAA".into(),
                input_type: InputType::Axis,
                id: 2,
            }),
        );
        p.set_script_variable(&a, "Speed", Some(&ScriptValue::Float(2.0)));
        p.set_script_name(&b, "Hat trim");
        let s = p.script_instances();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].language, ScriptLanguage::Lua);
        assert_eq!(s[0].source, ScriptSource::Code("print(1)\n".into()));
        assert_eq!(s[0].variables.len(), 2);
        assert_eq!(s[0].variables[0], ("Speed".into(), ScriptValue::Float(2.0)));
        assert_eq!(s[1].name, "Hat trim");
        assert_eq!(s[1].source, ScriptSource::File("trim.lua".into()));
        assert!(p.has_lua_scripts());

        assert_eq!(p.substitute_script_device("aaaa", "BBBB"), 1);
        assert!(matches!(
            &p.script_instance(&a).unwrap().variables[1].1,
            ScriptValue::PhysicalInput { device, .. } if device == "BBBB"
        ));
        p.set_script_source(&a, &ScriptSource::File("gear.lua".into()));
        assert_eq!(
            p.script_instance(&a).unwrap().source,
            ScriptSource::File("gear.lua".into())
        );
        p.move_script(&b, true);
        assert_eq!(p.script_instances()[0].id, b);
        p.set_script_variable(&a, "Speed", None);
        p.remove_script(&b);
        let s = p.script_instances();
        assert_eq!((s.len(), s[0].variables.len()), (1, 1));
    }
}
