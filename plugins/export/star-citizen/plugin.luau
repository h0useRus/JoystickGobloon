-- Star Citizen: bindings of virtual joysticks and keys in actionmaps.xml (the active profile)
-- and layout_<name>_exported.xml files (loaded in the game through Control Profiles).
--
-- Inputs as the game writes them: js<instance>_button<n>, js<n>_hat<h>_<up|right|down|left>,
-- js<n>_<x|y|z|rotx|roty|rotz|slider1|slider2>, kb1_<key>; "js1_ " (a space) unbinds.
-- Every vJoy device has the same product GUID, so the game tells them apart only by instance.

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
local function to_input(output, js_of)
	local key = string.match(output, "^key:0x(%x+)$")
	if key then
		local name = KEY_NAMES[tonumber(key, 16)]
		return if name then "kb1_" .. name else nil
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
	local key = string.match(input, "^kb1_([%w_]+)$")
	if key then
		local id = KEYS[key]
		return if id then string.format("key:0x%x", id) else nil
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
	local binds = {}
	for _, map in xml.elements(profiles, "actionmap") do
		for _, action in xml.elements(map, "action") do
			for _, rebind in xml.elements(action, "rebind") do
				local output = from_input(rebind.attrs.input or "", output_of)
				if output then
					table.insert(binds, { action = map.attrs.name .. "/" .. action.attrs.name, output = output })
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

	-- What each action gets: our inputs, and unbinds of the game's defaults on js1.
	-- Keys the profile sends are only shown: the player's keyboard layout stays theirs.
	local wanted = {}
	for _, b in ctx.binds do
		local input = if string.match(b.output, "^key:") then nil else to_input(b.output, js_of)
		if input then
			wanted[b.action] = wanted[b.action] or {}
			table.insert(wanted[b.action], input)
		end
	end
	local real = {}
	for id, inputs in wanted do
		table.sort(inputs)
		real[id] = table.concat(inputs, ", ")
	end
	if opts.clear_defaults and ours.js1 then
		for _, a in ctx.catalog.actions do
			if wanted[a.id] == nil then
				for _, d in a.default or {} do
					if string.match(d, "^js1_") then
						wanted[a.id] = { "js1_ " }
						break
					end
				end
			end
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
			-- Drop every binding on our devices, remembering them; unbinds stay unless the action
			-- gets an input on that device.
			local bound_on = {}
			for id, inputs in wanted do
				bound_on[id] = {}
				for _, input in inputs do
					bound_on[id][string.match(input, "^(js%d+)_") or ""] = true
				end
			end
			local old = {}
			for _, map in xml.elements(profiles, "actionmap") do
				for _, action in xml.elements(map, "action") do
					local id = map.attrs.name .. "/" .. action.attrs.name
					for _, rebind in xml.elements(action, "rebind") do
						local input = rebind.attrs.input or ""
						local js = string.match(input, "^(js%d+)_")
						if js and ours[js] then
							if not is_unbind(input) then
								old[id] = old[id] or {}
								table.insert(old[id], input)
								-- An input that stays keeps its element (activationMode, multiTap).
								if not table.find(wanted[id] or {}, input) then
									xml.remove(action, rebind)
								end
							elseif bound_on[id] and bound_on[id][js] then
								xml.remove(action, rebind)
							end
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
					for _, input in wanted[id] do
						if xml.find(action, "rebind", { input = input }) == nil then
							xml.append(action, xml.element("rebind", { input = input }))
						end
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
				table.insert(out, ' <options type="joystick" instance="' .. string.sub(l.game_device, 3) .. '" Product="' .. escape(product_of[l.output]) .. '"/>')
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
				table.insert(maps[map_name], { name = action_name, inputs = wanted[id] })
			end
		end
		for _, map_name in order do
			table.insert(out, ' <actionmap name="' .. escape(map_name) .. '">')
			for _, a in maps[map_name] do
				table.insert(out, '  <action name="' .. escape(a.name) .. '">')
				for _, input in a.inputs do
					table.insert(out, '   <rebind input="' .. escape(input) .. '"/>')
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
