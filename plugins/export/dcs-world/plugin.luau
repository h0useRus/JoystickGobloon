-- DCS World: every aircraft keeps its bindings in Saved Games/DCS[.<branch>]/Config/Input/<aircraft>/,
-- one file per device with only what differs from the aircraft's defaults:
--
--   joystick/<device name> {<DirectInput instance GUID>}.diff.lua, keyboard/Keyboard.diff.lua
--
--   local diff = {
--   	["keyDiffs"] = { ["d3001pnilu3001cd12vd1vpnilvu0"] = {
--   		["added"] = { [1] = { ["key"] = "JOY_BTN5" } },
--   		["removed"] = { [1] = { ["key"] = "JOY_BTN1" } },
--   		["name"] = "Landing gear" } },
--   	["axisDiffs"] = { ["a2001cdnil"] = { ["added"] = { [1] = { ["key"] = "JOY_Y",
--   		["filter"] = { ["invert"] = true, ... } } }, ["name"] = "Pitch" } },
--   }
--   return diff
--
-- The aircraft's actions and their defaults live in the game's own Lua code, so the list of
-- actions comes from the files the game writes with "Make HTML" in its controls screen
-- (Saved Games/DCS/InputLayoutsTxt/<aircraft>/<device>.html: a table of key combos, name,
-- category and id per action), and from the actions already changed in the diff files.
-- Each aircraft (an Input folder) is one target; action ids are "<aircraft>/<the game's id>".

-- Our axis names (output ids) <-> the game's.
local AXES = {
	x = "JOY_X", y = "JOY_Y", z = "JOY_Z", rx = "JOY_RX", ry = "JOY_RY", rz = "JOY_RZ",
	slider = "JOY_SLIDER1", dial = "JOY_SLIDER2",
}
local AXES_BACK = {}
for ours, game in AXES do
	AXES_BACK[game] = ours
end

-- Hat directions <-> the game's; diagonals are not bound one by one.
local HAT = { up = "U", right = "R", down = "D", left = "L" }
local HAT_BACK = {}
for ours, game in HAT do
	HAT_BACK[game] = ours
end

-- The game's key names <-> our key ids (extended << 8 | set 1 scan code).
local KEYS = {
	Esc = 0x01, ["1"] = 0x02, ["2"] = 0x03, ["3"] = 0x04, ["4"] = 0x05, ["5"] = 0x06,
	["6"] = 0x07, ["7"] = 0x08, ["8"] = 0x09, ["9"] = 0x0A, ["0"] = 0x0B, ["-"] = 0x0C,
	["="] = 0x0D, Back = 0x0E, Tab = 0x0F, Q = 0x10, W = 0x11, E = 0x12, R = 0x13, T = 0x14,
	Y = 0x15, U = 0x16, I = 0x17, O = 0x18, P = 0x19, ["["] = 0x1A, ["]"] = 0x1B, Enter = 0x1C,
	LCtrl = 0x1D, A = 0x1E, S = 0x1F, D = 0x20, F = 0x21, G = 0x22, H = 0x23, J = 0x24,
	K = 0x25, L = 0x26, [";"] = 0x27, ["'"] = 0x28, ["`"] = 0x29, LShift = 0x2A, ["\\"] = 0x2B,
	Z = 0x2C, X = 0x2D, C = 0x2E, V = 0x2F, B = 0x30, N = 0x31, M = 0x32, [","] = 0x33,
	["."] = 0x34, ["/"] = 0x35, RShift = 0x36, ["Num*"] = 0x37, LAlt = 0x38, Space = 0x39,
	CapsLock = 0x3A, F1 = 0x3B, F2 = 0x3C, F3 = 0x3D, F4 = 0x3E, F5 = 0x3F, F6 = 0x40,
	F7 = 0x41, F8 = 0x42, F9 = 0x43, F10 = 0x44, ScrollLock = 0x46, Num7 = 0x47, Num8 = 0x48,
	Num9 = 0x49, ["Num-"] = 0x4A, Num4 = 0x4B, Num5 = 0x4C, Num6 = 0x4D, ["Num+"] = 0x4E,
	Num1 = 0x4F, Num2 = 0x50, Num3 = 0x51, Num0 = 0x52, ["Num."] = 0x53, F11 = 0x57, F12 = 0x58,
	NumEnter = 0x11C, RCtrl = 0x11D, ["Num/"] = 0x135, SysRQ = 0x137, RAlt = 0x138,
	Pause = 0x45, NumLock = 0x145, Home = 0x147, Up = 0x148, PageUp = 0x149, Left = 0x14B,
	Right = 0x14D, End = 0x14F, Down = 0x150, PageDown = 0x151, Insert = 0x152, Delete = 0x153,
	LWin = 0x15B, RWin = 0x15C,
}
local KEY_NAMES = {}
for name, id in KEYS do
	KEY_NAMES[id] = name
end
-- Keys the game takes as modifiers ("reformers") of another key.
local MODIFIERS = {
	LCtrl = true, RCtrl = true, LShift = true, RShift = true, LAlt = true, RAlt = true,
	LWin = true, RWin = true,
}

-- A new axis binding's filter, as the game writes it.
local function new_filter(invert)
	return {
		curvature = { 0 },
		deadzone = 0,
		hardwareDetent = false,
		hardwareDetentAB = 0,
		hardwareDetentMax = 0,
		invert = invert,
		saturationX = 1,
		saturationY = 1,
		slider = false,
	}
end

local function slash(path)
	path = string.gsub(path, "\\", "/")
	if path ~= "" and string.sub(path, -1) ~= "/" then
		path ..= "/"
	end
	return path
end

-- ---- targets -----------------------------------------------------------------------------------

-- One target per aircraft folder under Saved Games/DCS*/Config/Input. `home` may hold `*`s of its
-- own (`stars` of them), which come first in the placeholders.
local function target_for(home, stars)
	local branch, aircraft = "{" .. (stars + 1) .. "}", "{" .. (stars + 2) .. "}"
	local saved = home .. "Saved Games/" .. branch .. "/"
	local input = saved .. "Config/Input/" .. aircraft .. "/"
	return {
		id = branch .. "/" .. aircraft,
		name = aircraft,
		each = home .. "Saved Games/DCS*/Config/Input/*",
		files = {
			keyboard = input .. "keyboard/Keyboard.diff.lua",
			log = saved .. "Logs/dcs.log",
		},
		dirs = { joystick = input .. "joystick" },
		globs = {
			joystick = input .. "joystick/*.diff.lua",
			html = saved .. "InputLayoutsTxt/" .. aircraft .. "/*.html",
		},
	}
end

plugin.targets(function(ctx)
	local env = ctx.env
	local found, seen = {}, {}
	local function add(home, stars)
		if home ~= "" and not seen[home] then
			seen[home] = true
			table.insert(found, target_for(home, stars))
		end
	end
	-- The user's folder: above the local application data folder, else above Documents.
	local lad = slash(env.local_app_data or "")
	local home = string.match(lad, "^(.*/)[Aa]pp[Dd]ata/[Ll]ocal/$")
	if home then
		add(home, 0)
	end
	local docs = slash(env.documents or "")
	local above = string.match(docs, "^(.*/)[^/]+/$")
	if above then
		add(above, 0)
	end
	if env.platform == "linux" then
		-- Proton and Wine prefixes.
		for _, root in env.roots do
			root = slash(root)
			add(root .. "drive_c/users/*/", 1)
			add(root .. "pfx/drive_c/users/*/", 1)
		end
	end
	return found
end)

-- dcs.log starts with the game's version: "... DCS/2.9.11.4686 (x86_64; ...".
plugin.version(function(ctx)
	local log = ctx.files.log
	if log == nil or log.text == nil then
		return nil
	end
	return string.match(string.sub(log.text, 1, 8192), "DCS/(%d+%.%d+[%d%.]*%d)")
end)

-- ---- files -------------------------------------------------------------------------------------

local function upper_guid(s)
	return if s then string.upper(s) else nil
end

-- The joystick diff files: { key, name = "vJoy Device", guid = "{...}" } by key order.
local function joystick_files(ctx)
	local list = {}
	for key, f in ctx.files do
		local path = string.match(key, "^joystick:(.+)$")
		if path then
			local file = string.match(path, "([^/\\]+)$") or path
			local name, guid = string.match(file, "^(.-)%s*(%b{})%.diff%.lua$")
			if guid then
				table.insert(list, { key = key, name = name, guid = upper_guid(guid), text = f.text })
			end
		end
	end
	table.sort(list, function(a, b)
		return a.key < b.key
	end)
	return list
end

-- The HTML files of the aircraft: { name, guid, text } (guid nil for the keyboard and mouse).
local function html_files(ctx)
	local list = {}
	for key, f in ctx.files do
		local path = string.match(key, "^html:(.+)$")
		if path and f.text then
			local file = string.match(path, "([^/\\]+)$") or path
			local stem = string.gsub(file, "%.[Hh][Tt][Mm][Ll]?$", "")
			local name, guid = string.match(stem, "^(.-)%s*(%b{})$")
			table.insert(list, { name = name or stem, guid = upper_guid(guid), text = f.text })
		end
	end
	return list
end

local ENTITIES = { amp = "&", lt = "<", gt = ">", quot = '"', apos = "'", nbsp = " " }

local function cell_text(s)
	s = string.gsub(s, "<[Bb][Rr]%s*/?>", "; ")
	s = string.gsub(s, "<[^>]*>", "")
	s = string.gsub(s, "&(#?%w+);", function(e)
		if string.sub(e, 1, 1) == "#" then
			local n = if string.sub(e, 2, 2) == "x" then tonumber(string.sub(e, 3), 16) else tonumber(string.sub(e, 2))
			return if n and n < 128 then string.char(n) else ""
		end
		return ENTITIES[e]
	end)
	s = string.gsub(s, "%s+", " ")
	return string.match(s, "^%s*(.-)%s*$")
end

-- The table of a "Make HTML" file: { {id, name, category, keys = {JOY_... = true}} }. Columns are
-- found by their headers ("Key combos", "Name", "Category", "ID").
local function html_actions(text)
	local low = string.lower(text)
	local rows = {}
	local at = 1
	while true do
		local s, e = string.find(low, "<tr[^>]*>", at)
		if s == nil then
			break
		end
		local close = string.find(low, "</tr>", e, true) or (string.find(low, "<tr", e + 1) or #low + 1)
		local cells, c = {}, e + 1
		while true do
			local cs, ce = string.find(low, "<t[dh][^>]*>", c)
			if cs == nil or cs > close then
				break
			end
			local ns = string.find(low, "</t[dh]>", ce + 1) or close
			local nx = string.find(low, "<t[dh][%s>]", ce + 1)
			local stop = math.min(ns, nx or ns, close)
			table.insert(cells, cell_text(string.sub(text, ce + 1, stop - 1)))
			c = stop
		end
		table.insert(rows, cells)
		at = close
	end
	local cols = nil
	local out = {}
	for _, cells in rows do
		if cols == nil then
			local found = {}
			for i, cell in cells do
				local h = string.lower(cell)
				if h == "name" then
					found.name = i
				elseif h == "id" or h == "identifier" then
					found.id = i
				elseif string.find(h, "categor", 1, true) then
					found.category = i
				elseif string.find(h, "combo", 1, true) or h == "keys" then
					found.combos = i
				end
			end
			if found.name and found.id then
				cols = found
			end
		else
			local id = cells[cols.id]
			if id and string.match(id, "^[ad][%w%.%-]+$") then
				local keys = {}
				for k in string.gmatch((cols.combos and cells[cols.combos]) or "", "JOY_[%w_]+") do
					keys[k] = true
				end
				table.insert(out, {
					id = id,
					name = cells[cols.name] or id,
					category = (cols.category and cells[cols.category]) or "",
					keys = keys,
				})
			end
		end
	end
	return out
end

-- A diff file's table; an empty one for a missing file.
local function load_diff(text)
	if text == nil or string.match(text, "^%s*$") then
		return {}
	end
	local ok, doc = pcall(luadata.parse, text)
	if not ok then
		error("A binding file of the game cannot be read: " .. tostring(doc))
	end
	return doc
end

local function save_diff(doc)
	return "local diff = " .. luadata.write(doc) .. "\nreturn diff\n"
end

-- The section of an action id: axes go to axisDiffs, everything else to keyDiffs.
local function section(id)
	return if string.sub(id, 1, 1) == "a" then "axisDiffs" else "keyDiffs"
end

local function list(t)
	local out = {}
	for _, v in t or {} do
		table.insert(out, v)
	end
	return out
end

local function sorted_keys(t)
	local keys = {}
	for k in t do
		table.insert(keys, k)
	end
	table.sort(keys)
	return keys
end

-- A combo without modifiers on a joystick: its key; nil for one with reformers.
local function plain(combo)
	if type(combo) ~= "table" or type(combo.key) ~= "string" then
		return nil
	end
	if combo.reformers and #list(combo.reformers) > 0 then
		return nil
	end
	return combo.key
end

-- ---- outputs <-> the game's keys ---------------------------------------------------------------

-- A joystick key ("JOY_BTN5") on the device `device` ("vjoy:1") -> our output id.
local function joy_output(device, key)
	local button = string.match(key, "^JOY_BTN(%d+)$")
	if button then
		return device .. ":button:" .. button
	end
	local hat, dir = string.match(key, "^JOY_BTN_POV(%d+)_(%a+)$")
	if hat and HAT_BACK[dir] then
		return device .. ":hat:" .. hat .. ":" .. HAT_BACK[dir]
	end
	if AXES_BACK[key] then
		return device .. ":axis:" .. AXES_BACK[key]
	end
	return nil
end

-- Our output id -> (device, joystick key).
local function joy_key(output)
	local device, rest = string.match(output, "^(vjoy:%d+):(.+)$")
	if device == nil then
		return nil
	end
	local button = string.match(rest, "^button:(%d+)$")
	if button then
		return device, "JOY_BTN" .. button
	end
	local hat, dir = string.match(rest, "^hat:(%d+):(%a+)$")
	if hat and HAT[dir] then
		return device, "JOY_BTN_POV" .. hat .. "_" .. HAT[dir]
	end
	local axis = string.match(rest, "^axis:(%w+)$")
	if axis and AXES[axis] then
		return device, AXES[axis]
	end
	return device, nil
end

local function is_keys(output)
	return string.match(output, "^key:") ~= nil or string.match(output, "^mouse:") ~= nil
end

-- A keyboard combo of the game -> our output id ("key:0x1d+key:0x22").
local function combo_output(combo)
	if type(combo) ~= "table" or KEYS[combo.key] == nil then
		return nil
	end
	local parts = {}
	for _, r in list(combo.reformers) do
		if KEYS[r] == nil then
			return nil
		end
		table.insert(parts, string.format("key:0x%x", KEYS[r]))
	end
	table.insert(parts, string.format("key:0x%x", KEYS[combo.key]))
	return table.concat(parts, "+")
end

-- Our keys output -> a keyboard combo of the game; nil for what the game's keyboard cannot take
-- (mouse buttons, unknown keys).
local function output_combo(output)
	local names = {}
	for _, part in string.split(output, "+") do
		local key = string.match(part, "^key:0x(%x+)$")
		local name = key and KEY_NAMES[tonumber(key, 16)]
		if name == nil then
			return nil
		end
		table.insert(names, name)
	end
	local key = table.remove(names)
	-- The other keys are the modifiers held with it.
	for _, n in names do
		if not MODIFIERS[n] then
			return nil
		end
	end
	local combo = { key = key }
	if #names > 0 then
		combo.reformers = names
	end
	return combo
end

local function combo_text(combo)
	local parts = list(combo.reformers)
	table.insert(parts, combo.key)
	return table.concat(parts, " + ")
end

-- ---- devices -----------------------------------------------------------------------------------

-- Our devices the game has files for: by the DirectInput instance GUID in the file name, else by
-- name when exactly one of ours and one file have it.
local function found_links(ctx, files)
	local links, used = {}, {}
	for _, ours in ctx.ours do
		if ours.instance and ours.instance ~= "" then
			used[ours.output] = true
			table.insert(links, { output = ours.output, game_device = upper_guid(ours.instance) })
		end
	end
	for _, ours in ctx.ours do
		if not used[ours.output] then
			local same, file = 0, nil
			for _, other in ctx.ours do
				if string.lower(other.name) == string.lower(ours.name) then
					same += 1
				end
			end
			local n = 0
			for _, f in files do
				if string.lower(f.name) == string.lower(ours.name) then
					n += 1
					file = f
				end
			end
			if same == 1 and n == 1 then
				table.insert(links, { output = ours.output, game_device = file.guid })
			end
		end
	end
	return links
end

-- Device links: those the profile keeps, else those found. output -> GUID.
local function links_of(ctx, files)
	local guid_of = {}
	for _, l in found_links(ctx, files) do
		guid_of[l.output] = l.game_device
	end
	for _, l in ctx.devices or {} do
		local g = string.match(l.game_device or "", "%b{}")
		if g then
			guid_of[l.output] = string.upper(g)
		end
	end
	return guid_of
end

-- ---- defaults ----------------------------------------------------------------------------------

-- An action's keys on a joystick, from a diff entry: added (plain), removed (plain), the added
-- entries by key and the entries with modifiers (kept as they are).
local function entry_keys(entry)
	local added, removed, by_key, other = {}, {}, {}, {}
	for _, c in list(entry and entry.added) do
		local k = plain(c)
		if k then
			added[k] = true
			by_key[k] = c
		else
			table.insert(other, c)
		end
	end
	for _, c in list(entry and entry.removed) do
		local k = plain(c)
		if k then
			removed[k] = true
		end
	end
	return added, removed, by_key, other
end

-- The game's defaults of each action on a device: what its HTML file shows less what the diff
-- added, plus what the diff removed. Without the HTML file only the removed ones are known.
-- Also what the HTML file shows, by action.
local function defaults_of(doc, html)
	local out, shown = {}, {}
	local function at(id)
		out[id] = out[id] or {}
		return out[id]
	end
	for _, a in html or {} do
		local entry = (doc[section(a.id)] or {})[a.id]
		local added = entry_keys(entry)
		shown[a.id] = a.keys
		for k in a.keys do
			if not added[k] then
				at(a.id)[k] = true
			end
		end
	end
	for _, sec in { "keyDiffs", "axisDiffs" } do
		for id, entry in doc[sec] or {} do
			local _, removed = entry_keys(entry)
			for k in removed do
				at(id)[k] = true
			end
		end
	end
	return out, shown
end

local function html_for(htmls, guid)
	for _, h in htmls do
		if h.guid == guid then
			return h
		end
	end
	return nil
end

-- ---- read --------------------------------------------------------------------------------------

plugin.read(function(ctx)
	local aircraft = ctx.target.name
	local prefix = aircraft .. "/"
	local files = joystick_files(ctx)
	local htmls = html_files(ctx)
	local links = found_links(ctx, files)
	local output_of = {}
	for output, guid in links_of(ctx, files) do
		output_of[guid] = output
	end

	-- The actions: every HTML file's, then the changed ones the files name.
	local actions, known = {}, {}
	local parsed = {}
	for _, h in htmls do
		parsed[h] = html_actions(h.text)
		for _, a in parsed[h] do
			if not known[a.id] then
				known[a.id] = true
				table.insert(actions, {
					id = prefix .. a.id,
					name = a.name,
					category = a.category,
					kind = if section(a.id) == "axisDiffs" then "axis" else "button",
				})
			end
		end
	end
	local function name_changed(doc)
		for _, sec in { "keyDiffs", "axisDiffs" } do
			for _, id in sorted_keys(doc[sec] or {}) do
				local entry = doc[sec][id]
				if not known[id] and type(entry) == "table" then
					known[id] = true
					table.insert(actions, {
						id = prefix .. id,
						name = if type(entry.name) == "string" then entry.name else id,
						category = "Other",
						kind = if sec == "axisDiffs" then "axis" else "button",
					})
				end
			end
		end
	end

	local binds = {}
	for _, f in files do
		local doc = load_diff(f.text)
		name_changed(doc)
		local device = output_of[f.guid]
		if device then
			local h = html_for(htmls, f.guid)
			local defaults = defaults_of(doc, h and parsed[h])
			local ids = {}
			for id in defaults do
				ids[id] = true
			end
			for _, sec in { "keyDiffs", "axisDiffs" } do
				for id in doc[sec] or {} do
					ids[id] = true
				end
			end
			for _, id in sorted_keys(ids) do
				local added, removed, by_key = entry_keys((doc[section(id)] or {})[id])
				local keys = {}
				for k in defaults[id] or {} do
					if not removed[k] then
						keys[k] = true
					end
				end
				for k in added do
					keys[k] = true
				end
				for _, k in sorted_keys(keys) do
					local output = joy_output(device, k)
					if output then
						local filter = by_key[k] and by_key[k].filter
						table.insert(binds, {
							action = prefix .. id,
							output = output,
							invert = type(filter) == "table" and filter.invert == true,
							default = not added[k],
						})
					end
				end
			end
		end
	end

	local kb = ctx.files.keyboard
	local doc = load_diff(kb and kb.text)
	name_changed(doc)
	for _, id in sorted_keys(doc.keyDiffs or {}) do
		for _, c in list(doc.keyDiffs[id].added) do
			local output = combo_output(c)
			if output then
				table.insert(binds, { action = prefix .. id, output = output })
			end
		end
	end
	return { devices = links, binds = binds, actions = actions }
end)

-- ---- write -------------------------------------------------------------------------------------

local function same_set(a, b)
	for k in a do
		if not b[k] then
			return false
		end
	end
	for k in b do
		if not a[k] then
			return false
		end
	end
	return true
end

local function joined(set)
	return table.concat(sorted_keys(set), ", ")
end

plugin.write(function(ctx)
	local aircraft = ctx.target.name
	local prefix = aircraft .. "/"
	local files = joystick_files(ctx)
	local htmls = html_files(ctx)
	local guid_of = links_of(ctx, files)
	local name_of = {}
	for _, h in htmls do
		for _, a in html_actions(h.text) do
			name_of[a.id] = name_of[a.id] or a.name
		end
	end

	-- What the profile binds, per device: id -> { key -> invert }; and the keyboard combos of
	-- the actions it holds them for.
	local wanted, combos, keyed, unlinked = {}, {}, {}, false
	for _, id in ctx.cleared or {} do
		if string.sub(id, 1, #prefix) == prefix then
			keyed[string.sub(id, #prefix + 1)] = true
		end
	end
	for _, b in ctx.binds do
		if string.sub(b.action, 1, #prefix) == prefix then
			local id = string.sub(b.action, #prefix + 1)
			if is_keys(b.output) then
				keyed[id] = true
				local combo = output_combo(b.output)
				if combo and section(id) == "keyDiffs" then
					combos[id] = combos[id] or {}
					table.insert(combos[id], combo)
				else
					plugin.log(id .. ": the game's keyboard cannot take " .. b.output)
				end
			else
				local device, key = joy_key(b.output)
				if device and key then
					local axis = AXES_BACK[key] ~= nil
					if axis ~= (section(id) == "axisDiffs") then
						plugin.log(id .. ": " .. b.output .. " does not fit this action (axis and button)")
					elseif guid_of[device] == nil then
						unlinked = true
					else
						wanted[device] = wanted[device] or {}
						wanted[device][id] = wanted[device][id] or {}
						wanted[device][id][key] = b.invert == true
					end
				end
			end
		end
	end
	if unlinked then
		error("The game's files do not name the virtual joysticks yet. Connect them, start the game once, or type each device's GUID ({...}) above the table.")
	end

	local out, create, changes = {}, {}, {}
	local function change(id, before, after)
		local from, to = joined(before), joined(after)
		if from == to then
			return
		end
		local action = prefix .. id
		if from == "" then
			table.insert(changes, { kind = "added", action = action, to = to })
		elseif to == "" then
			table.insert(changes, { kind = "removed", action = action, from = from })
		else
			table.insert(changes, { kind = "changed", action = action, from = from, to = to })
		end
	end

	-- The joystick file of each of our devices.
	for _, device in sorted_keys(guid_of) do
		local guid = guid_of[device]
		local file = nil
		for _, f in files do
			if f.guid == guid then
				file = f
			end
		end
		local doc = load_diff(file and file.text)
		local h = html_for(htmls, guid)
		local defaults, shown = defaults_of(doc, h and html_actions(h.text))
		local want = wanted[device] or {}
		local ids = {}
		for id in defaults do
			ids[id] = true
		end
		for id in want do
			ids[id] = true
		end
		for _, sec in { "keyDiffs", "axisDiffs" } do
			for id in doc[sec] or {} do
				ids[id] = true
			end
		end
		local touched = false
		for _, id in sorted_keys(ids) do
			local sec = section(id)
			local entry = (doc[sec] or {})[id]
			local added, removed, by_key, other = entry_keys(entry)
			local dflt = defaults[id] or {}
			local before = {}
			for k in dflt do
				if not removed[k] then
					before[k] = true
				end
			end
			for k in added do
				before[k] = true
			end
			local after = {}
			for k in want[id] or {} do
				after[k] = true
			end
			-- Inverted where the game's binding is not: written as added with its filter.
			local function inverted(k)
				local f = by_key[k] and by_key[k].filter
				return type(f) == "table" and f.invert == true
			end
			local flips = false
			for k, inv in want[id] or {} do
				if sec == "axisDiffs" and inv ~= inverted(k) and (inv or by_key[k]) then
					flips = true
				end
			end
			if not same_set(before, after) or flips then
				touched = true
				change(id, before, after)
				-- Every key the entry adds or removes stays known from the diff itself, so a
				-- "Make HTML" table older than the diff reads right: a default put back is
				-- written as added, an added key the table shows that goes is removed.
				local new_added, gone = {}, {}
				for _, k in sorted_keys(after) do
					local inv = want[id][k]
					local live = dflt[k] and not removed[k]
					-- A default in effect stays the game's unless it is inverted or has the
					-- player's curve (then it is removed and added again with its filter).
					local tuned = by_key[k] ~= nil and type(by_key[k].filter) == "table"
					if not live or (sec == "axisDiffs" and (inv or tuned)) then
						local c = by_key[k] or { key = k }
						if sec == "axisDiffs" then
							if type(c.filter) == "table" then
								c.filter.invert = inv
							elseif inv then
								c.filter = new_filter(true)
							end
						end
						table.insert(new_added, c)
						if live then
							gone[k] = true
						end
					end
				end
				for _, c in other do
					table.insert(new_added, c)
				end
				for k in dflt do
					if not after[k] then
						gone[k] = true
					end
				end
				for k in added do
					if (shown[id] or {})[k] and not after[k] then
						gone[k] = true
					end
				end
				local new_removed = {}
				for _, k in sorted_keys(gone) do
					table.insert(new_removed, { key = k })
				end
				doc[sec] = doc[sec] or {}
				if #new_added == 0 and #new_removed == 0 then
					doc[sec][id] = nil
				else
					local e = entry or {}
					e.added = if #new_added > 0 then new_added else nil
					e.removed = if #new_removed > 0 then new_removed else nil
					e.name = e.name or name_of[id]
					doc[sec][id] = e
				end
			end
		end
		if touched then
			local text = save_diff(doc)
			if file then
				out[file.key] = text
			else
				local name = "vJoy Device"
				for _, o in ctx.ours do
					if o.output == device and o.name ~= "" then
						name = o.name
					end
				end
				table.insert(create, { dir = "joystick", name = name .. " " .. guid .. ".diff.lua", text = text })
			end
		end
	end

	-- The keyboard: only the actions the profile holds keys for.
	if next(keyed) ~= nil and ctx.files.keyboard then
		local doc = load_diff(ctx.files.keyboard.text)
		doc.keyDiffs = doc.keyDiffs or {}
		local touched = false
		for _, id in sorted_keys(keyed) do
			if section(id) == "keyDiffs" then
				local entry = doc.keyDiffs[id]
				local before, after, keep = {}, {}, {}
				for _, c in list(entry and entry.added) do
					local text = combo_text(c)
					if combo_output(c) then
						before[text] = c
					else
						table.insert(keep, c)
					end
				end
				for _, c in combos[id] or {} do
					after[combo_text(c)] = c
				end
				if not same_set(before, after) then
					touched = true
					change(id, before, after)
					local new_added = keep
					for _, text in sorted_keys(after) do
						table.insert(new_added, before[text] or after[text])
					end
					local removed = list(entry and entry.removed)
					if #new_added == 0 and #removed == 0 then
						doc.keyDiffs[id] = nil
					else
						local e = entry or {}
						e.added = if #new_added > 0 then new_added else nil
						e.name = e.name or name_of[id]
						doc.keyDiffs[id] = e
					end
				end
			end
		end
		if touched then
			out.keyboard = save_diff(doc)
		end
	end

	return { files = out, create = create, changes = changes }
end)
