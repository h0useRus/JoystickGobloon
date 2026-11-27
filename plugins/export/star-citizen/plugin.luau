-- Star Citizen: bindings of virtual joysticks, keyboard and mouse in actionmaps.xml (the active
-- profile) and layout_<name>_exported.xml files (loaded in the game through Control Profiles).
--
-- Inputs as the game writes them: js<instance>_button<n>, js<n>_hat<h>_<up|right|down|left>,
-- js<n>_<x|y|z|rotx|roty|rotz|slider1|slider2>, kb1_<key>, kb1_<key>+<key>, mo1_<button>;
-- "js1_ " (a space) unbinds. A rebind on a device replaces the game's default on that device.
-- A rebind's activationMode (or multiTap) says how it is pressed: delayed_press* and
-- delayed_hold* = held long (our "@long"), double_tap* or multiTap="2" = pressed twice ("@double").
-- Every vJoy device has the same product GUID, so the game tells them apart only by instance.
-- An axis is inverted per joystick instance and option group (the catalog's `invert`):
-- <options type="joystick" instance="1" ...><flight_move_pitch invert="1"/></options>.

local CHANNELS = { "LIVE", "PTU", "EPTU", "TECH-PREVIEW", "HOTFIX" }
local INSTALLS = {
	"",
	"StarCitizen/",
	"RSI/StarCitizen/",
	"Roberts Space Industries/StarCitizen/",
	"Program Files/Roberts Space Industries/StarCitizen/",
	"Games/Roberts Space Industries/StarCitizen/",
	"Games/RSI/StarCitizen/",
	"Games/StarCitizen/",
}

-- Our axis names (output ids) <-> the game's.
local AXES = { x = "x", y = "y", z = "z", rx = "rotx", ry = "roty", rz = "rotz", slider = "slider1", dial = "slider2" }
local AXES_BACK = {}
for ours, game in AXES do
	AXES_BACK[game] = ours
end

-- The game's key names <-> our key ids (extended << 8 | set 1 scan code).
local KEYS = {
	escape = 0x01, ["1"] = 0x02, ["2"] = 0x03, ["3"] = 0x04, ["4"] = 0x05, ["5"] = 0x06,
	["6"] = 0x07, ["7"] = 0x08, ["8"] = 0x09, ["9"] = 0x0A, ["0"] = 0x0B, minus = 0x0C,
	equals = 0x0D, backspace = 0x0E, tab = 0x0F, q = 0x10, w = 0x11, e = 0x12, r = 0x13,
	t = 0x14, y = 0x15, u = 0x16, i = 0x17, o = 0x18, p = 0x19, lbracket = 0x1A,
	rbracket = 0x1B, enter = 0x1C, lctrl = 0x1D, a = 0x1E, s = 0x1F, d = 0x20, f = 0x21,
	g = 0x22, h = 0x23, j = 0x24, k = 0x25, l = 0x26, semicolon = 0x27, apostrophe = 0x28,
	grave = 0x29, lshift = 0x2A, backslash = 0x2B, z = 0x2C, x = 0x2D, c = 0x2E, v = 0x2F,
	b = 0x30, n = 0x31, m = 0x32, comma = 0x33, period = 0x34, slash = 0x35, rshift = 0x36,
	np_multiply = 0x37, lalt = 0x38, space = 0x39, capslock = 0x3A, f1 = 0x3B, f2 = 0x3C,
	f3 = 0x3D, f4 = 0x3E, f5 = 0x3F, f6 = 0x40, f7 = 0x41, f8 = 0x42, f9 = 0x43, f10 = 0x44,
	scrolllock = 0x46, np_7 = 0x47, np_8 = 0x48, np_9 = 0x49, np_subtract = 0x4A,
	np_4 = 0x4B, np_5 = 0x4C, np_6 = 0x4D, np_add = 0x4E, np_1 = 0x4F, np_2 = 0x50,
	np_3 = 0x51, np_0 = 0x52, np_period = 0x53, f11 = 0x57, f12 = 0x58,
	np_enter = 0x11C, rctrl = 0x11D, np_divide = 0x135, print = 0x137, ralt = 0x138,
	numlock = 0x145, home = 0x147, up = 0x148, pgup = 0x149, left = 0x14B, right = 0x14D,
	["end"] = 0x14F, down = 0x150, pgdn = 0x151, insert = 0x152, delete = 0x153,
}
local KEY_NAMES = {}
for name, id in KEYS do
	KEY_NAMES[id] = name
end

-- Our mouse button names (output ids) <-> the game's.
local MOUSE = {
	left = "mouse1", right = "mouse2", middle = "mouse3", back = "mouse4", forward = "mouse5",
	["wheel-up"] = "mwheel_up", ["wheel-down"] = "mwheel_down",
}
local MOUSE_BACK = {}
for ours, game in MOUSE do
	MOUSE_BACK[game] = ours
end

plugin.options({
	update_actionmaps = { kind = "bool", label = "Also update actionmaps.xml", default = true },
	layout = { kind = "bool", label = "Write a layout file", default = true },
	layout_name = { kind = "text", label = "Layout name", default = "{profile}" },
	clear_defaults = {
		kind = "bool",
		label = "Unbind the game's own joystick bindings on js1",
		default = true,
	},
})

local function slash(path)
	path = string.gsub(path, "\\", "/")
	if path ~= "" and string.sub(path, -1) ~= "/" then
		path ..= "/"
	end
	return path
end

plugin.targets(function(ctx)
	local found, seen = {}, {}
	for _, root in ctx.env.roots do
		root = slash(root)
		for _, install in INSTALLS do
			for _, channel in CHANNELS do
				local base = root .. install .. channel
				if not seen[base] then
					seen[base] = true
					local user = base .. "/user/client/0"
					table.insert(found, {
						id = base,
						name = channel,
						files = {
							manifest = base .. "/build_manifest.id",
							actionmaps = user .. "/Profiles/default/actionmaps.xml",
						},
						dirs = { mappings = user .. "/controls/mappings" },
						version_file = "manifest",
					})
				end
			end
		end
	end
	return found
end)

plugin.version(function(ctx)
	local f = ctx.files.manifest
	if f == nil or f.text == nil then
		return nil
	end
	local ok, manifest = pcall(json.parse, f.text)
	if ok and type(manifest) == "table" and type(manifest.Data) == "table" then
		return manifest.Data.Version
	end
	return nil
end)

-- ---- outputs <-> inputs ----------------------------------------------------------------------

-- Our output id -> the game's input, through device links (output "vjoy:1" -> "js1").
local function is_keys(output)
	return string.match(output, "^key:") ~= nil or string.match(output, "^mouse:") ~= nil
end

-- Keys and mouse buttons pressed together -> "kb1_ralt+y", "mo1_mouse2", "kb1_lalt+mouse2".
local function keys_input(output)
	local names, mouse_only = {}, true
	for _, part in string.split(output, "+") do
		local key = string.match(part, "^key:0x(%x+)$")
		local button = string.match(part, "^mouse:([%a%-]+)$")
		local name = nil
		if key then
			name = KEY_NAMES[tonumber(key, 16)]
			mouse_only = false
		elseif button then
			name = MOUSE[button]
		end
		if name == nil then
			return nil
		end
		table.insert(names, name)
	end
	return (if mouse_only then "mo1_" else "kb1_") .. table.concat(names, "+")
end

-- The device of an input: "js2", "kb1", "mo1".
local function device_of(input)
	return string.match(input, "^(%a+%d+)_")
end

-- "kb1_u+lshift" and "kb1_lshift+u" are one input: the parts in one order.
local function canon(input)
	local dev, rest = string.match(input, "^(%a+%d+)_(.+)$")
	if (dev ~= "kb1" and dev ~= "mo1") or string.find(rest, "+", 1, true) == nil then
		return input
	end
	local parts = string.split(rest, "+")
	table.sort(parts)
	return dev .. "_" .. table.concat(parts, "+")
end

local function to_input(output, js_of)
	if is_keys(output) then
		return keys_input(output)
	end
	local device, rest = string.match(output, "^(vjoy:%d+):(.+)$")
	local js = device and js_of[device]
	if js == nil then
		return nil
	end
	local button = string.match(rest, "^button:(%d+)$")
	if button then
		return js .. "_button" .. button
	end
	local hat, dir = string.match(rest, "^hat:(%d+):(%a+)$")
	if hat then
		return js .. "_hat" .. hat .. "_" .. dir
	end
	local axis = string.match(rest, "^axis:(%w+)$")
	if axis and AXES[axis] then
		return js .. "_" .. AXES[axis]
	end
	return nil
end

-- The game's input -> our output id; nil for devices that are not ours and for unbinds.
local function from_input(input, output_of)
	local dev, keys = string.match(input, "^(%a+%d+)_(.+)$")
	if dev == "kb1" or dev == "mo1" then
		local parts = {}
		for _, name in string.split(keys, "+") do
			if KEYS[name] then
				table.insert(parts, string.format("key:0x%x", KEYS[name]))
			elseif MOUSE_BACK[name] then
				table.insert(parts, "mouse:" .. MOUSE_BACK[name])
			else
				return nil
			end
		end
		return table.concat(parts, "+")
	end
	local js, rest = string.match(input, "^(js%d+)_(.+)$")
	local device = js and output_of[js]
	if device == nil then
		return nil
	end
	local button = string.match(rest, "^button(%d+)$")
	if button then
		return device .. ":button:" .. button
	end
	local hat, dir = string.match(rest, "^hat(%d+)_(%a+)$")
	if hat then
		return device .. ":hat:" .. hat .. ":" .. dir
	end
	if AXES_BACK[rest] then
		return device .. ":axis:" .. AXES_BACK[rest]
	end
	return nil
end

-- How a rebind is pressed: "long", "double" or nil (as the action says).
local function press_of(rebind)
	local mode = rebind.attrs.activationMode or ""
	if string.match(mode, "^delayed_press") or string.match(mode, "^delayed_hold") then
		return "long"
	end
	if string.match(mode, "^double_tap") or rebind.attrs.multiTap == "2" then
		return "double"
	end
	return nil
end

-- The activationMode we write for a press.
local MODES = { long = "delayed_press", double = "double_tap" }

-- Makes a rebind pressed as `press` (nil: as the action says), keeping a mode it has already.
local function set_press(rebind, press)
	if press_of(rebind) == press then
		return
	end
	rebind.attrs.activationMode = press and MODES[press] or nil
	rebind.attrs.multiTap = nil
end

-- "vjoy:1:button:5@long" -> "vjoy:1:button:5", "long".
local function split_press(output)
	local base, press = string.match(output, "^(.-)@(%a+)$")
	if base and MODES[press] then
		return base, press
	end
	return output, nil
end

-- An input with its press, for the change list: "js1_button5 (long)".
local function shown(input, press)
	if press then
		return input .. " (" .. press .. ")"
	end
	return input
end

local function is_vjoy(device)
	return string.match(device.output, "^vjoy:%d+$") ~= nil
end

local function product(device)
	return device.name .. "  " .. device.guid
end

-- Device links: those given, else our virtual joysticks as js1, js2... in order.
local function links_of(ctx)
	local js_of, output_of, list = {}, {}, {}
	local given = ctx.devices or {}
	if #given == 0 then
		local n = 0
		for _, d in ctx.ours do
			if is_vjoy(d) then
				n += 1
				table.insert(given, { output = d.output, game_device = "js" .. n })
			end
		end
	end
	for _, l in given do
		if string.match(l.game_device, "^js%d+$") then
			js_of[l.output] = l.game_device
			output_of[l.game_device] = l.output
			table.insert(list, l)
		end
	end
	return js_of, output_of, list
end

-- The element holding the action maps: ActionProfiles in actionmaps.xml, the root in layouts.
local function profiles_of(doc)
	local root = xml.root(doc)
	if root == nil then
		return nil
	end
	return xml.find(root, "ActionProfiles") or root
end

-- ---- inverted axes -----------------------------------------------------------------------------

-- Action id -> the option group that inverts it.
local function invert_groups(catalog)
	local out = {}
	for _, a in catalog.actions do
		if a.invert then
			out[a.id] = a.invert
		end
	end
	return out
end

-- Whether a joystick input is an axis ("js1_x", not a button or hat).
local function is_axis_input(input)
	local rest = string.match(input, "^js%d+_(.+)$")
	return rest ~= nil and AXES_BACK[rest] ~= nil
end

-- The joystick options of an instance ("1"), if the file has them.
local function joystick_options(profiles, instance)
	return xml.find(profiles, "options", { type = "joystick", instance = instance })
end

-- Whether option group `group` is inverted in `opt`.
local function is_inverted(opt, group)
	local el = opt and xml.find(opt, group)
	return el ~= nil and el.attrs.invert == "1"
end

-- The whitespace in front of `child` among `parent`'s children.
local function indent_of(parent, child)
	local list = parent.children
	for i, c in list do
		if c == child then
			local before = list[i - 1]
			if type(before) == "string" and string.match(before, "^%s*$") then
				return before
			end
		end
	end
	return "\n "
end

-- Sets or clears the invert of `group` in `opt`, indented one step deeper than `opt`.
local function set_inverted(profiles, opt, group, on)
	local el = xml.find(opt, group)
	if on then
		if el then
			el.attrs.invert = "1"
			return
		end
		el = xml.element(group, { invert = "1" }, { "invert" })
		if #xml.elements(opt) == 0 then
			local outer = indent_of(profiles, opt)
			opt.children = { outer .. " ", el, outer }
		else
			xml.append(opt, el)
		end
	elseif el and el.attrs.invert == "1" then
		el.attrs.invert = nil
		if next(el.attrs) == nil and #xml.elements(el) == 0 then
			xml.remove(opt, el)
			if #xml.elements(opt) == 0 then
				opt.children = {}
			end
		end
	end
end

-- ---- read --------------------------------------------------------------------------------------

plugin.read(function(ctx)
	local f = ctx.files.actionmaps
	if f == nil or f.text == nil then
		return {}
	end
	local doc = xml.parse(f.text)
	local profiles = profiles_of(doc)
	if profiles == nil then
		return {}
	end
	-- Our devices by the product the game wrote, in instance order.
	local devices, output_of, used = {}, {}, {}
	for _, opt in xml.elements(profiles, "options") do
		local instance = tonumber(opt.attrs.instance)
		local prod = opt.attrs.Product
		if opt.attrs.type == "joystick" and instance and prod then
			for _, d in ctx.ours do
				if is_vjoy(d) and not used[d.output] and string.find(string.upper(prod), string.upper(d.guid), 1, true) then
					used[d.output] = true
					local js = "js" .. instance
					output_of[js] = d.output
					table.insert(devices, { output = d.output, game_device = js })
					break
				end
			end
		end
	end
	-- The player's bindings; a rebind on the keyboard or mouse (an unbind too) replaces the
	-- game's default there.
	local groups = invert_groups(ctx.catalog)
	local binds, rebound = {}, {}
	for _, map in xml.elements(profiles, "actionmap") do
		for _, action in xml.elements(map, "action") do
			local id = map.attrs.name .. "/" .. action.attrs.name
			for _, rebind in xml.elements(action, "rebind") do
				local input = rebind.attrs.input or ""
				local dev = device_of(input)
				if dev == "kb1" or dev == "mo1" then
					rebound[id] = rebound[id] or {}
					rebound[id][dev] = true
				end
				local output = from_input(input, output_of)
				if output then
					local press = press_of(rebind)
					if press and not is_keys(output) then
						output ..= "@" .. press
					end
					local invert = nil
					if groups[id] and is_axis_input(input) then
						local opt = joystick_options(profiles, string.match(input, "^js(%d+)_"))
						invert = is_inverted(opt, groups[id]) or nil
					end
					table.insert(binds, { action = id, output = output, invert = invert })
				end
			end
		end
	end
	for _, a in ctx.catalog.actions do
		for _, d in a.default or {} do
			local dev = device_of(d)
			if (dev == "kb1" or dev == "mo1") and not (rebound[a.id] and rebound[a.id][dev]) then
				local output = from_input(d, output_of)
				if output then
					table.insert(binds, { action = a.id, output = output, default = true })
				end
			end
		end
	end
	return { devices = devices, binds = binds }
end)

-- ---- write -------------------------------------------------------------------------------------

local function split_id(id)
	return string.match(id, "^([^/]+)/(.+)$")
end

local function escape(s)
	s = string.gsub(s, "&", "&amp;")
	s = string.gsub(s, "<", "&lt;")
	s = string.gsub(s, ">", "&gt;")
	s = string.gsub(s, '"', "&quot;")
	return s
end

local function sorted_keys(t)
	local keys = {}
	for k in t do
		table.insert(keys, k)
	end
	table.sort(keys)
	return keys
end

-- Inserts `child` right after `after` among `parent`'s children, indented like it.
local function insert_after(parent, after, child)
	local list = parent.children
	for i, c in list do
		if c == after then
			local indent = if type(list[i - 1]) == "string" then list[i - 1] else nil
			table.insert(list, i + 1, child)
			if indent then
				table.insert(list, i + 1, indent)
			end
			return
		end
	end
	xml.append(parent, child)
end

local function is_unbind(input)
	return string.sub(input, -2) == "_ "
end

plugin.write(function(ctx)
	local opts = ctx.options
	local js_of, _, links = links_of(ctx)
	local ours = {}
	for _, l in links do
		ours[l.game_device] = true
	end
	local product_of = {}
	for _, d in ctx.ours do
		product_of[d.output] = product(d)
	end

	-- What each action gets: our joystick inputs, and the keyboard and mouse inputs of the
	-- actions the profile holds them for (`keyed`; the other actions keep the game's).
	local keyed = {}
	for _, id in ctx.cleared or {} do
		keyed[id] = true
	end
	local groups = invert_groups(ctx.catalog)
	-- Option groups of the axes the profile binds on our devices: js -> group -> inverted.
	local inverts, inverted_id = {}, {}
	local wanted, press_in = {}, {}
	for _, b in ctx.binds do
		if is_keys(b.output) then
			keyed[b.action] = true
		end
		local base, press = split_press(b.output)
		local input = to_input(base, js_of)
		local group = groups[b.action]
		if input and group and is_axis_input(input) then
			local js = device_of(input)
			inverts[js] = inverts[js] or {}
			inverts[js][group] = inverts[js][group] or b.invert or false
			if b.invert then
				inverted_id[b.action] = true
			end
		end
		if input then
			wanted[b.action] = wanted[b.action] or {}
			table.insert(wanted[b.action], input)
			press_in[b.action] = press_in[b.action] or {}
			press_in[b.action][canon(input)] = press
		end
	end
	local function press_for(id, input)
		return press_in[id] and press_in[id][canon(input)]
	end
	local real, bound_on = {}, {}
	for id, inputs in wanted do
		local list = {}
		bound_on[id] = {}
		for _, input in inputs do
			local text = shown(canon(input), press_for(id, input))
			if inverted_id[id] and groups[id] and is_axis_input(input) then
				text ..= " (inverted)"
			end
			table.insert(list, text)
			bound_on[id][device_of(input) or ""] = true
		end
		table.sort(list)
		real[id] = table.concat(list, ", ")
	end
	-- Unbinds where the game's own default would come back: on keyboard and mouse for the
	-- actions held here, on js1 for every action when asked.
	local defaults = {}
	for _, a in ctx.catalog.actions do
		for _, d in a.default or {} do
			local dev = device_of(d)
			if dev then
				defaults[a.id] = defaults[a.id] or {}
				defaults[a.id][dev] = defaults[a.id][dev] or {}
				table.insert(defaults[a.id][dev], canon(d))
			end
		end
	end
	local function unbind(id, dev)
		if defaults[id] and defaults[id][dev] and not (bound_on[id] and bound_on[id][dev]) then
			wanted[id] = wanted[id] or {}
			table.insert(wanted[id], dev .. "_ ")
		end
	end
	for _, id in sorted_keys(keyed) do
		unbind(id, "kb1")
		unbind(id, "mo1")
	end
	if opts.clear_defaults and ours.js1 then
		for _, a in ctx.catalog.actions do
			unbind(a.id, "js1")
		end
	end

	local files, create, changes = {}, {}, {}
	local f = ctx.files.actionmaps
	if opts.update_actionmaps and f ~= nil and f.text ~= nil then
		local doc = xml.parse(f.text)
		local profiles = profiles_of(doc)
		if profiles ~= nil then
			-- Our devices' products on their instances.
			local options = xml.elements(profiles, "options")
			for _, l in links do
				local instance = string.sub(l.game_device, 3)
				local opt = xml.find(profiles, "options", { type = "joystick", instance = instance })
				if opt then
					opt.attrs.Product = product_of[l.output] or opt.attrs.Product
				else
					opt = xml.element("options", { type = "joystick", instance = instance, Product = product_of[l.output] }, { "type", "instance", "Product" })
					insert_after(profiles, options[#options], opt)
					table.insert(options, opt)
				end
			end
			-- Drop every binding on our devices, and on keyboard and mouse for the actions held
			-- here, remembering them; unbinds stay unless the action gets an input on that device.
			local want = {}
			for id, inputs in wanted do
				want[id] = {}
				for _, input in inputs do
					want[id][canon(input)] = true
				end
			end
			local old, in_file = {}, {}
			for _, map in xml.elements(profiles, "actionmap") do
				for _, action in xml.elements(map, "action") do
					local id = map.attrs.name .. "/" .. action.attrs.name
					for _, rebind in xml.elements(action, "rebind") do
						local input = rebind.attrs.input or ""
						local dev = device_of(input)
						if dev and (ours[dev] or ((dev == "kb1" or dev == "mo1") and keyed[id])) then
							in_file[id] = in_file[id] or {}
							in_file[id][dev] = true
							if not is_unbind(input) then
								old[id] = old[id] or {}
								local text = shown(canon(input), press_of(rebind))
								if groups[id] and is_axis_input(input) then
									if is_inverted(joystick_options(profiles, string.sub(dev, 3)), groups[id]) then
										text ..= " (inverted)"
									end
								end
								table.insert(old[id], text)
								-- An input that stays keeps its element (activationMode, multiTap)
								-- unless it is pressed another way now.
								if not (want[id] and want[id][canon(input)]) then
									xml.remove(action, rebind)
								else
									set_press(rebind, press_for(id, input))
								end
							elseif bound_on[id] and bound_on[id][dev] then
								xml.remove(action, rebind)
							end
						end
					end
				end
			end
			-- Before, a held action without a rebind on keyboard or mouse had the game's default.
			for id in keyed do
				for _, dev in { "kb1", "mo1" } do
					if defaults[id] and defaults[id][dev] and not (in_file[id] and in_file[id][dev]) then
						old[id] = old[id] or {}
						for _, d in defaults[id][dev] do
							table.insert(old[id], d)
						end
					end
				end
			end
			-- Add ours, into the elements already there where they are.
			for _, id in sorted_keys(wanted) do
				local map_name, action_name = split_id(id)
				if map_name then
					local map = xml.find(profiles, "actionmap", { name = map_name })
						or xml.append(profiles, xml.element("actionmap", { name = map_name }))
					local action = xml.find(map, "action", { name = action_name })
						or xml.append(map, xml.element("action", { name = action_name }))
					local have = {}
					for _, rebind in xml.elements(action, "rebind") do
						have[canon(rebind.attrs.input or "")] = true
					end
					for _, input in wanted[id] do
						if not have[canon(input)] then
							local press = press_for(id, input)
							xml.append(
								action,
								xml.element("rebind", { input = input, activationMode = press and MODES[press] or nil }, { "input", "activationMode" })
							)
						end
					end
				end
			end
			-- Inverted axes as the profile has them, on our devices only.
			for js, by_group in inverts do
				local opt = ours[js] and joystick_options(profiles, string.sub(js, 3))
				if opt then
					for _, group in sorted_keys(by_group) do
						set_inverted(profiles, opt, group, by_group[group])
					end
				end
			end
			-- Drop what was left empty.
			for _, map in xml.elements(profiles, "actionmap") do
				for _, action in xml.elements(map, "action") do
					if #xml.elements(action) == 0 then
						xml.remove(map, action)
					end
				end
				if #xml.elements(map) == 0 then
					xml.remove(profiles, map)
				end
			end
			files.actionmaps = xml.write(doc)
			-- Changes of real bindings.
			local ids = {}
			for id in old do
				ids[id] = true
			end
			for id in real do
				ids[id] = true
			end
			for _, id in sorted_keys(ids) do
				local before = nil
				if old[id] then
					table.sort(old[id])
					before = table.concat(old[id], ", ")
				end
				local after = real[id]
				if before == nil then
					table.insert(changes, { kind = "added", action = id, to = after })
				elseif after == nil then
					table.insert(changes, { kind = "removed", action = id, from = before })
				elseif before ~= after then
					table.insert(changes, { kind = "changed", action = id, from = before, to = after })
				end
			end
		end
	end

	if opts.layout then
		local name = string.gsub(opts.layout_name or "{profile}", "{profile}", ctx.profile)
		name = string.gsub(name, "[^%w_%-]", "_")
		if name == "" then
			name = "JoystickGobloon"
		end
		local out = {
			'<ActionMaps version="1" optionsVersion="2" rebindVersion="2" profileName="' .. escape(name) .. '">',
			' <CustomisationUIHeader label="' .. escape(name) .. '" description="" image="">',
			"  <devices>",
			'   <keyboard instance="1"/>',
			'   <mouse instance="1"/>',
		}
		for _, l in links do
			table.insert(out, '   <joystick instance="' .. string.sub(l.game_device, 3) .. '"/>')
		end
		table.insert(out, "  </devices>")
		table.insert(out, " </CustomisationUIHeader>")
		for _, l in links do
			if product_of[l.output] then
				local head = ' <options type="joystick" instance="' .. string.sub(l.game_device, 3) .. '" Product="' .. escape(product_of[l.output]) .. '"'
				local on = {}
				for _, group in sorted_keys(inverts[l.game_device] or {}) do
					if inverts[l.game_device][group] then
						table.insert(on, group)
					end
				end
				if #on == 0 then
					table.insert(out, head .. "/>")
				else
					table.insert(out, head .. ">")
					for _, group in on do
						table.insert(out, "  <" .. group .. ' invert="1"/>')
					end
					table.insert(out, " </options>")
				end
			end
		end
		table.insert(out, " <modifiers />")
		local maps, order = {}, {}
		for _, id in sorted_keys(wanted) do
			local map_name, action_name = split_id(id)
			if map_name then
				if maps[map_name] == nil then
					maps[map_name] = {}
					table.insert(order, map_name)
				end
				table.insert(maps[map_name], { id = id, name = action_name, inputs = wanted[id] })
			end
		end
		for _, map_name in order do
			table.insert(out, ' <actionmap name="' .. escape(map_name) .. '">')
			for _, a in maps[map_name] do
				table.insert(out, '  <action name="' .. escape(a.name) .. '">')
				for _, input in a.inputs do
					local press = press_for(a.id, input)
					local mode = if press then ' activationMode="' .. MODES[press] .. '"' else ""
					table.insert(out, '   <rebind input="' .. escape(input) .. '"' .. mode .. "/>")
				end
				table.insert(out, "  </action>")
			end
			table.insert(out, " </actionmap>")
		end
		table.insert(out, "</ActionMaps>")
		table.insert(create, {
			dir = "mappings",
			name = "layout_" .. name .. "_exported.xml",
			text = table.concat(out, "\n") .. "\n",
		})
	end

	return { files = files, create = create, changes = changes }
end)
