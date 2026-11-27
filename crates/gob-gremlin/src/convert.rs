//! [`Element`] tree <-> [`Profile`]: Joystick Gobloon profiles and the Gremlin R14+ profiles
//! they grew from.

use gob_core::{
    Action, ActionConfig, Device, Element, InputBinding, Item, Layer, Mode, Modifier, Node,
    Profile, Property, Section, VirtualDevices, VirtualJoystick, VoiceCommand,
};

use crate::Error;

/// First profile format version written by Gremlin R14.
pub const MIN_VERSION: u32 = 14;
/// `app` attribute of Joystick Gobloon's profiles.
pub const APP: &str = "Gobloon";
/// Version of Joystick Gobloon's profile format this build writes and the newest it reads.
pub const FORMAT_VERSION: u32 = 1;
/// Axis names of virtual joysticks by slot (vJoy's), as `<joystick axes="...">` lists them.
const AXIS_NAMES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "Sl0", "Sl1"];

/// Joystick Gremlin Ex writes its own format (grown from Gremlin R13) with version numbers
/// that look like R14+: devices carry their inputs (`<device device-guid=".." name="..">` with
/// modes inside) and the root names the start mode.
fn is_gremlin_ex(root: &Element) -> bool {
    root.attr("start_mode").is_some()
        || root.attr("default_mode").is_some()
        || named(root, "devices")
            .any(|d| named(d, "device").any(|d| d.attr("device-guid").is_some()))
        || named(root, "vjoy-devices").next().is_some()
}

pub fn profile_from_element(root: &Element) -> Result<Profile, Error> {
    if root.name != "profile" {
        return Err(Error::NotAProfile);
    }
    let version = root.attr("version").unwrap_or_default().to_string();
    let number = version.parse::<u32>().ok();
    let source = match root.attr("app") {
        Some(APP) => {
            if number.is_none_or(|v| v > FORMAT_VERSION) {
                return Err(Error::NewerFormat(version));
            }
            format!("Joystick Gobloon {version}")
        }
        Some(_) => return Err(Error::NotAProfile),
        None => {
            if is_gremlin_ex(root) {
                return Err(Error::GremlinEx(version));
            }
            if number.is_none_or(|v| v < MIN_VERSION) {
                return Err(Error::UnsupportedVersion(version));
            }
            format!("Joystick Gremlin {version}")
        }
    };
    let mut p = Profile {
        source,
        order: Vec::new(),
        modes: Vec::new(),
        ..Default::default()
    };
    for section in root.elements() {
        let kind = match section.name.as_str() {
            "inputs" => {
                p.inputs = named(section, "input").map(input_from).collect();
                Section::Inputs
            }
            "settings" => {
                p.settings = section.clone();
                Section::Settings
            }
            "logical-device" => {
                p.logical_device = section.clone();
                Section::LogicalDevice
            }
            "library" => {
                p.library = named(section, "action").map(action_from).collect();
                Section::Library
            }
            "modes" => {
                p.modes = named(section, "mode")
                    .map(|m| Mode {
                        name: m.text(),
                        parent: m.attr("parent").map(String::from),
                    })
                    .collect();
                Section::Modes
            }
            "modifiers" => {
                p.modifiers = named(section, "modifier")
                    .map(|m| {
                        let attr = |n: &str| m.attr(n).unwrap_or_default().to_string();
                        Modifier {
                            name: attr("name"),
                            device_id: attr("device-id"),
                            input_type: attr("input-type"),
                            input_id: attr("input-id"),
                        }
                    })
                    .collect();
                Section::Modifiers
            }
            "scripts" => {
                p.scripts = section.clone();
                Section::Scripts
            }
            "devices" => {
                p.devices = named(section, "device")
                    .map(|d| Device {
                        id: d.child_text("device-id").unwrap_or_default(),
                        name: d.child_text("device-name").unwrap_or_default(),
                    })
                    .collect();
                Section::Devices
            }
            "virtual-devices" => {
                p.virtual_devices = virtual_from(section);
                Section::VirtualDevices
            }
            "voice-commands" => {
                p.voice_commands = voice_from(section);
                Section::VoiceCommands
            }
            "key-combos" => {
                p.key_combos = combos_from(section);
                Section::KeyCombos
            }
            "game-bindings" => {
                p.game_bindings = games_from(section);
                Section::GameBindings
            }
            "osc-inputs" => {
                p.osc_inputs = osc_from(section);
                Section::OscInputs
            }
            "picture-cards" => {
                p.picture_cards = cards_from(section);
                Section::PictureCards
            }
            _ => {
                p.extra.push(section.clone());
                Section::Extra(p.extra.len() - 1)
            }
        };
        p.order.push(kind);
    }
    Ok(p)
}

pub fn profile_to_element(p: &Profile) -> Element {
    // Gremlin's header while Joystick Gremlin can still open the profile.
    let mut root = if p.needs_own_format() {
        Element::new("profile")
            .with_attr("app", APP)
            .with_attr("version", FORMAT_VERSION.to_string())
    } else {
        Element::new("profile").with_attr("version", MIN_VERSION.to_string())
    };
    for section in &p.order {
        let empty = match section {
            Section::Modifiers => p.modifiers.is_empty(),
            Section::VirtualDevices => p.virtual_devices.is_empty(),
            Section::VoiceCommands => p.voice_commands.is_empty(),
            Section::KeyCombos => p.key_combos.is_empty(),
            Section::GameBindings => p.game_bindings.is_empty(),
            Section::OscInputs => p.osc_inputs.is_empty(),
            Section::PictureCards => p.picture_cards.is_empty(),
            _ => false,
        };
        if empty {
            continue;
        }
        root.push(match *section {
            Section::Inputs => container("inputs", p.inputs.iter().map(input_to)),
            Section::Settings => p.settings.clone(),
            Section::LogicalDevice => p.logical_device.clone(),
            Section::Library => container("library", p.library.iter().map(action_to)),
            Section::Modes => container(
                "modes",
                p.modes.iter().map(|m| {
                    let el = Element::with_text("mode", &m.name);
                    match &m.parent {
                        Some(parent) => el.with_attr("parent", parent),
                        None => el,
                    }
                }),
            ),
            Section::Modifiers => container(
                "modifiers",
                p.modifiers.iter().map(|m| {
                    Element::new("modifier")
                        .with_attr("name", &m.name)
                        .with_attr("device-id", &m.device_id)
                        .with_attr("input-type", &m.input_type)
                        .with_attr("input-id", &m.input_id)
                }),
            ),
            Section::Scripts => p.scripts.clone(),
            Section::Devices => container(
                "devices",
                p.devices.iter().map(|d| {
                    container(
                        "device",
                        [leaf("device-id", &d.id), leaf("device-name", &d.name)],
                    )
                }),
            ),
            Section::VirtualDevices => virtual_to(&p.virtual_devices),
            Section::VoiceCommands => voice_to(p),
            Section::KeyCombos => combos_to(&p.key_combos),
            Section::GameBindings => games_to(&p.game_bindings),
            Section::OscInputs => osc_to(&p.osc_inputs),
            Section::PictureCards => cards_to(&p.picture_cards),
            Section::Extra(i) => p.extra[i].clone(),
        });
    }
    root
}

/// `<virtual-devices>`: `<joystick id="1" axes="X Y Rz" buttons="32" hats="1"
/// hat-type="continuous" name="Throttle" pid="0xBE01"/>` (name and pid only when set) and
/// `<gamepad id="1"/>`; unknown axis names are skipped.
fn virtual_from(section: &Element) -> VirtualDevices {
    let number = |e: &Element, name: &str| e.attr(name).and_then(|v| v.trim().parse().ok());
    VirtualDevices {
        joysticks: named(section, "joystick")
            .filter_map(|j| {
                Some(VirtualJoystick {
                    id: number(j, "id")?,
                    axes: j
                        .attr("axes")
                        .unwrap_or_default()
                        .split_whitespace()
                        .filter_map(|a| {
                            let i = AXIS_NAMES.iter().position(|n| n.eq_ignore_ascii_case(a))?;
                            Some(i as u8 + 1)
                        })
                        .collect(),
                    buttons: number(j, "buttons").unwrap_or(0),
                    hats: number(j, "hats").unwrap_or(0),
                    discrete_hats: j.attr("hat-type") == Some("discrete"),
                    name: j
                        .attr("name")
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .map(str::to_string),
                    product: j.attr("pid").and_then(|p| {
                        let p = p.trim();
                        let hex = p.strip_prefix("0x").or_else(|| p.strip_prefix("0X"))?;
                        u16::from_str_radix(hex, 16).ok()
                    }),
                })
            })
            .collect(),
        gamepads: named(section, "gamepad")
            .filter_map(|g| number(g, "id"))
            .collect(),
    }
}

fn virtual_to(v: &VirtualDevices) -> Element {
    let joysticks = v.joysticks.iter().map(|j| {
        let axes: Vec<&str> = j
            .axes
            .iter()
            .filter_map(|&s| AXIS_NAMES.get(usize::from(s).checked_sub(1)?).copied())
            .collect();
        let mut el = Element::new("joystick")
            .with_attr("id", j.id.to_string())
            .with_attr("axes", axes.join(" "))
            .with_attr("buttons", j.buttons.to_string())
            .with_attr("hats", j.hats.to_string())
            .with_attr(
                "hat-type",
                if j.discrete_hats {
                    "discrete"
                } else {
                    "continuous"
                },
            );
        if let Some(name) = &j.name {
            el = el.with_attr("name", name.clone());
        }
        if let Some(pid) = j.product {
            el = el.with_attr("pid", format!("0x{pid:04X}"));
        }
        el
    });
    let gamepads = v
        .gamepads
        .iter()
        .map(|id| Element::new("gamepad").with_attr("id", id.to_string()));
    container("virtual-devices", joysticks.chain(gamepads))
}

/// `<voice-commands>`: `<command id="1" hold="0.1"><phrase>lights on</phrase>...</command>`,
/// and `<group id="2" hold="0.1"><phrase>radio</phrase><command .../><group .../></group>` around
/// commands and groups; ones without a valid id are skipped (with what is in them), a missing or
/// bad hold is the default.
fn voice_from(section: &Element) -> Vec<VoiceCommand> {
    fn read(parent_el: &Element, parent: Option<u32>, commands: &mut Vec<VoiceCommand>) {
        for c in parent_el.elements() {
            let group = match c.name.as_str() {
                "command" => false,
                "group" => true,
                _ => continue,
            };
            let Some(id) = c.attr("id").and_then(|v| v.trim().parse().ok()) else {
                continue;
            };
            if id == 0 || commands.iter().any(|o| o.id == id) {
                continue;
            }
            commands.push(VoiceCommand {
                id,
                phrases: named(c, "phrase")
                    .map(|p| p.text().split_whitespace().collect::<Vec<_>>().join(" "))
                    .filter(|p| !p.is_empty())
                    .collect(),
                hold: c
                    .attr("hold")
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|h| h.is_finite() && *h > 0.0)
                    .unwrap_or(gob_core::DEFAULT_VOICE_HOLD),
                parent,
                group,
            });
            if group {
                read(c, Some(id), commands);
            }
        }
    }
    let mut commands = Vec::new();
    read(section, None, &mut commands);
    commands
}

/// `<voice-commands>` in tree order: every command and group once, whatever its parent says (one
/// whose group is gone, or whose groups make a loop, at the top level, as [`Profile::voice_tree`]
/// lists them), at any depth.
fn voice_to(p: &Profile) -> Element {
    fn node(c: &VoiceCommand, inner: Vec<Element>) -> Element {
        let phrases = c.phrases.iter().map(|p| Element::with_text("phrase", p));
        container(
            if c.group { "group" } else { "command" },
            phrases.chain(inner),
        )
        .with_attr("id", c.id.to_string())
        .with_attr("hold", c.hold.to_string())
    }
    // The elements at `depth` from `tree[*at..]`, each with what follows it one level deeper.
    fn level(tree: &[(usize, &VoiceCommand)], at: &mut usize, depth: usize) -> Vec<Element> {
        let mut out = Vec::new();
        while let Some(&(d, c)) = tree.get(*at) {
            if d < depth {
                break;
            }
            *at += 1;
            let inner = level(tree, at, d + 1);
            out.push(node(c, inner));
        }
        out
    }
    let tree = p.voice_tree();
    let mut at = 0;
    container("voice-commands", level(&tree, &mut at, 0))
}

/// `<game-bindings>`: `<game id="..." target="..." version="..."><device output="vjoy:1"
/// game-device="js1"/><bind action="..." output="vjoy:1:button:5" invert="true"/><clear
/// action="..."/></game>`; binds with an unknown output and games without an id are skipped.
fn games_from(section: &Element) -> Vec<gob_core::GameBindings> {
    let mut games: Vec<gob_core::GameBindings> = Vec::new();
    for g in named(section, "game") {
        let game = g.attr("id").unwrap_or_default().trim().to_string();
        if game.is_empty() || games.iter().any(|o| o.game == game) {
            continue;
        }
        let devices = named(g, "device")
            .filter_map(|d| {
                Some(gob_core::GameDevice {
                    output: d.attr("output")?.trim().to_string(),
                    game_device: d.attr("game-device")?.trim().to_string(),
                })
            })
            .collect();
        let binds = named(g, "bind")
            .filter_map(|b| {
                Some(gob_core::GameBind {
                    action: b.attr("action")?.trim().to_string(),
                    output: gob_core::GameOutput::parse(b.attr("output")?)?,
                    invert: b.attr("invert").is_some_and(|v| v.trim() == "true"),
                })
            })
            .collect();
        games.push(gob_core::GameBindings {
            game,
            target: g.attr("target").unwrap_or_default().trim().to_string(),
            version: g
                .attr("version")
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            devices,
            binds,
            cleared: named(g, "clear")
                .filter_map(|c| Some(c.attr("action")?.trim().to_string()))
                .collect(),
        });
    }
    games
}

fn games_to(games: &[gob_core::GameBindings]) -> Element {
    container(
        "game-bindings",
        games.iter().map(|g| {
            let devices = g.devices.iter().map(|d| {
                Element::new("device")
                    .with_attr("output", &d.output)
                    .with_attr("game-device", &d.game_device)
            });
            let binds = g.binds.iter().map(|b| {
                let el = Element::new("bind")
                    .with_attr("action", &b.action)
                    .with_attr("output", b.output.to_string());
                if b.invert {
                    el.with_attr("invert", "true")
                } else {
                    el
                }
            });
            let cleared = g
                .cleared
                .iter()
                .map(|a| Element::new("clear").with_attr("action", a));
            let mut el = container("game", devices.chain(binds).chain(cleared))
                .with_attr("id", &g.game)
                .with_attr("target", &g.target);
            if let Some(v) = &g.version {
                el = el.with_attr("version", v);
            }
            el
        }),
    )
}

/// `<key-combos>`: `<combo id="1"><key scan-code="29" extended="false"/>...</combo>`;
/// combinations without a valid id or with fewer than two keys are skipped.
fn combos_from(section: &Element) -> Vec<gob_core::KeyCombo> {
    let mut combos: Vec<gob_core::KeyCombo> = Vec::new();
    for c in named(section, "combo") {
        let Some(id) = c.attr("id").and_then(|v| v.trim().parse().ok()) else {
            continue;
        };
        let mut keys: Vec<gob_core::Key> = Vec::new();
        for k in named(c, "key") {
            let Some(scan_code) = k.attr("scan-code").and_then(|v| v.trim().parse().ok()) else {
                continue;
            };
            let key = gob_core::Key {
                scan_code,
                extended: k.attr("extended").is_some_and(|v| v.trim() == "true"),
            };
            if !keys.contains(&key) && keys.len() < gob_core::MAX_COMBO_KEYS {
                keys.push(key);
            }
        }
        if id == 0 || keys.len() < 2 || combos.iter().any(|o| o.id == id) {
            continue;
        }
        combos.push(gob_core::KeyCombo { id, keys });
    }
    combos
}

fn combos_to(combos: &[gob_core::KeyCombo]) -> Element {
    container(
        "key-combos",
        combos.iter().map(|c| {
            container(
                "combo",
                c.keys.iter().map(|k| {
                    Element::new("key")
                        .with_attr("scan-code", k.scan_code.to_string())
                        .with_attr("extended", if k.extended { "true" } else { "false" })
                }),
            )
            .with_attr("id", c.id.to_string())
        }),
    )
}

/// `<picture-cards>`: `<card plugin="vkb-x" view="main" control="hat-2" side="left" y="120"/>`.
/// Cards without a plugin, view, control or height, or placed already, are skipped.
fn cards_from(section: &Element) -> Vec<gob_core::PictureCard> {
    let mut cards: Vec<gob_core::PictureCard> = Vec::new();
    for e in named(section, "card") {
        let text = |name: &str| {
            e.attr(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let (Some(plugin), Some(view), Some(control)) =
            (text("plugin"), text("view"), text("control"))
        else {
            continue;
        };
        let Some(y) = e
            .attr("y")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|y| y.is_finite())
        else {
            continue;
        };
        let side = if e.attr("side").is_some_and(|v| v.trim() == "right") {
            gob_core::CardSide::Right
        } else {
            gob_core::CardSide::Left
        };
        if cards
            .iter()
            .any(|c| c.plugin == plugin && c.view == view && c.control == control)
        {
            continue;
        }
        cards.push(gob_core::PictureCard {
            plugin,
            view,
            control,
            side,
            y,
        });
    }
    cards
}

fn cards_to(cards: &[gob_core::PictureCard]) -> Element {
    container(
        "picture-cards",
        cards.iter().map(|c| {
            Element::new("card")
                .with_attr("plugin", &c.plugin)
                .with_attr("view", &c.view)
                .with_attr("control", &c.control)
                .with_attr(
                    "side",
                    match c.side {
                        gob_core::CardSide::Left => "left",
                        gob_core::CardSide::Right => "right",
                    },
                )
                .with_attr("y", c.y.round().to_string())
        }),
    )
}

/// `<osc-inputs>`: `<input id="1" address="/deck/fire" type="button"/>`, axes with
/// `low="0" high="1"`. Inputs without an id or address, or with one already taken, are skipped.
fn osc_from(section: &Element) -> Vec<gob_core::OscInput> {
    let mut inputs: Vec<gob_core::OscInput> = Vec::new();
    for e in named(section, "input") {
        let Some(id) = e.attr("id").and_then(|v| v.trim().parse().ok()) else {
            continue;
        };
        let Some(address) = e.attr("address").and_then(gob_core::osc_address) else {
            continue;
        };
        if id == 0 || inputs.iter().any(|o| o.id == id || o.address == address) {
            continue;
        }
        let kind = e
            .attr("type")
            .and_then(gob_core::OscKind::parse)
            .unwrap_or_default();
        let mut input = gob_core::OscInput::new(id, &address, kind);
        let number = |name: &str| {
            e.attr(name)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        if let (Some(low), Some(high)) = (number("low"), number("high"))
            && low != high
        {
            (input.low, input.high) = (low, high);
        }
        inputs.push(input);
    }
    inputs
}

fn osc_to(inputs: &[gob_core::OscInput]) -> Element {
    container(
        "osc-inputs",
        inputs.iter().map(|i| {
            let el = Element::new("input")
                .with_attr("id", i.id.to_string())
                .with_attr("address", &i.address)
                .with_attr("type", i.kind.input_type());
            match i.kind {
                gob_core::OscKind::Axis => el
                    .with_attr("low", i.low.to_string())
                    .with_attr("high", i.high.to_string()),
                gob_core::OscKind::Button => el,
            }
        }),
    )
}

fn named<'a>(parent: &'a Element, name: &'a str) -> impl Iterator<Item = &'a Element> {
    parent.elements().filter(move |e| e.name == name)
}

fn container(name: &str, children: impl IntoIterator<Item = Element>) -> Element {
    let mut el = Element::new(name);
    children.into_iter().for_each(|c| el.push(c));
    el
}

/// Text element; empty text becomes `<name/>`.
fn leaf(name: &str, text: &str) -> Element {
    if text.is_empty() {
        Element::new(name)
    } else {
        Element::with_text(name, text)
    }
}

fn input_from(e: &Element) -> InputBinding {
    let mut b = InputBinding {
        device_id: String::new(),
        input_type: String::new(),
        mode: String::new(),
        input_id: String::new(),
        configs: Vec::new(),
        extra: Vec::new(),
    };
    for c in e.elements() {
        match c.name.as_str() {
            "device-id" => b.device_id = c.text(),
            "input-type" => b.input_type = c.text(),
            "mode" => b.mode = c.text(),
            "input-id" => b.input_id = c.text(),
            "action-configuration" => b.configs.push(config_from(c)),
            _ => b.extra.push(c.clone()),
        }
    }
    b
}

fn config_from(e: &Element) -> ActionConfig {
    let mut cfg = ActionConfig {
        root_action: String::new(),
        behavior: String::new(),
        virtual_button: None,
        layer: Layer::parse(e.attr("layer").unwrap_or_default()),
        additive: e.attr("additive").is_some_and(|v| v == "true"),
        keep_value: e.attr("keep-value").is_some_and(|v| v == "true"),
        send_state: e.attr("send-state").is_some_and(|v| v == "true"),
        extra: Vec::new(),
    };
    for c in e.elements() {
        match c.name.as_str() {
            "root-action" => cfg.root_action = c.text(),
            "behavior" => cfg.behavior = c.text(),
            "virtual-button" => cfg.virtual_button = Some(c.clone()),
            _ => cfg.extra.push(c.clone()),
        }
    }
    cfg
}

fn input_to(b: &InputBinding) -> Element {
    let configs = b.configs.iter().map(|cfg| {
        let mut el = container(
            "action-configuration",
            [
                leaf("root-action", &cfg.root_action),
                leaf("behavior", &cfg.behavior),
            ]
            .into_iter()
            .chain(cfg.virtual_button.iter().cloned())
            .chain(cfg.extra.iter().cloned()),
        );
        if !cfg.layer.is_base() {
            el = el.with_attr("layer", cfg.layer.to_string());
        }
        if cfg.additive {
            el = el.with_attr("additive", "true");
        }
        if cfg.keep_value {
            el = el.with_attr("keep-value", "true");
        }
        if cfg.send_state {
            el = el.with_attr("send-state", "true");
        }
        el
    });
    container(
        "input",
        [
            leaf("device-id", &b.device_id),
            leaf("input-type", &b.input_type),
            leaf("mode", &b.mode),
            leaf("input-id", &b.input_id),
        ]
        .into_iter()
        .chain(configs)
        .chain(b.extra.iter().cloned()),
    )
}

fn action_from(e: &Element) -> Action {
    Action {
        id: e.attr("id").unwrap_or_default().into(),
        kind: e.attr("type").unwrap_or_default().into(),
        items: e.elements().map(item_from).collect(),
    }
}

fn action_to(a: &Action) -> Element {
    let el = Element::new("action")
        .with_attr("id", &a.id)
        .with_attr("type", &a.kind);
    container_items(el, &a.items)
}

fn is_leaf(e: &Element) -> bool {
    e.elements().next().is_none()
}

fn item_from(e: &Element) -> Item {
    if e.name == "property" {
        if let Some(p) = property_from(e) {
            return Item::Property(p);
        }
    } else if e.name == "action-id" && is_leaf(e) {
        return Item::ActionRef(e.text());
    } else if !is_leaf(e) || e.children.is_empty() {
        let has_text = e.children.iter().any(|n| matches!(n, Node::Text(_)));
        if !has_text {
            return Item::Group {
                tag: e.name.clone(),
                attrs: e.attrs.clone(),
                items: e.elements().map(item_from).collect(),
            };
        }
    }
    Item::Raw(e.clone())
}

/// Accepts only the exact `<property type><name/><value/></property>` shape.
fn property_from(e: &Element) -> Option<Property> {
    let [(key, ty)] = e.attrs.as_slice() else {
        return None;
    };
    let kids: Vec<&Element> = e.elements().collect();
    let [name, value] = kids.as_slice() else {
        return None;
    };
    (key == "type"
        && name.name == "name"
        && value.name == "value"
        && is_leaf(name)
        && is_leaf(value))
    .then(|| Property {
        ty: ty.clone(),
        name: name.text(),
        value: value.text(),
    })
}

fn item_to(item: &Item) -> Element {
    match item {
        Item::Property(p) => {
            container("property", [leaf("name", &p.name), leaf("value", &p.value)])
                .with_attr("type", &p.ty)
        }
        Item::ActionRef(id) => leaf("action-id", id),
        Item::Group { tag, attrs, items } => {
            let mut el = Element::new(tag);
            el.attrs = attrs.clone();
            container_items(el, items)
        }
        Item::Raw(e) => e.clone(),
    }
}

fn container_items(mut el: Element, items: &[Item]) -> Element {
    items.iter().for_each(|i| el.push(item_to(i)));
    el
}
