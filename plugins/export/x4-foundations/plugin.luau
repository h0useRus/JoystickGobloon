-- X4: Foundations: the active bindings in <Documents>/Egosoft/X4/<Steam account>/inputmap.xml
-- (GOG: <Documents>/Egosoft/X4/inputmap.xml; Linux: ~/.config/EgoSoft/X4/<id>/), one line per
-- input of a control, in three sections:
--
--   <action id="INPUT_ACTION_TOGGLE_AUTOPILOT" source="INPUT_SOURCE_JOYBUTTONS_2" code="INPUT_XBUTTON_14"/>
--   <state id="INPUT_STATE_BOOST" source="INPUT_SOURCE_KEYBOARD" code="INPUT_KEYCODE_TAB"/>
--   <range id="INPUT_RANGE_THROTTLE" source="INPUT_SOURCE_JOYAXES_2" code="INPUT_JOYAXIS_Z"/>
--
-- A joystick source names a slot (none = 1, _2 ... _8), not a device: config.xml next to the file
-- pins devices to slots by their DirectInput instance GUID once the player picks them in Settings >
-- Controls (<joystick num="<slot - 1>">{guid}</joystick>). Buttons 1-11 carry the names of an Xbox
-- controller's (A, B, X, Y, LEFT_SHOULDER, RIGHT_SHOULDER, BACK, START, LEFT_THUMB, RIGHT_THUMB,
-- BIGBUTTON), 12-64 their numbers; hat 1 is DPAD_<direction>; axes X, Y, Z, RX, RY, RZ, SLIDER1,
-- SLIDER2. Keys are DirectInput names with _SHIFT, _CONTROL or _SHIFT_CONTROL for the keys held.
-- Half axes (sgn), joystick modifiers (_MOD1) and diagonals are kept as they are, never written.
-- The game writes these files at the end of a session: export while it is not running.

local NAMED_BUTTONS = {
	"A", "B", "X", "Y", "LEFT_SHOULDER", "RIGHT_SHOULDER", "BACK", "START", "LEFT_THUMB",
	"RIGHT_THUMB", "BIGBUTTON",
}
local NAMED_BACK = {}
for n, name in NAMED_BUTTONS do
	NAMED_BACK[name] = n
end
local MAX_BUTTON = 64

local AXES = {
	x = "X", y = "Y", z = "Z", rx = "RX", ry = "RY", rz = "RZ", slider = "SLIDER1", dial = "SLIDER2",
}
local AXES_BACK = {}
for ours, game in AXES do
	AXES_BACK[game] = ours
end

local HAT = { up = "DPAD_UP", right = "DPAD_RIGHT", down = "DPAD_DOWN", left = "DPAD_LEFT" }
local HAT_BACK = {}
for ours, game in HAT do
	HAT_BACK[game] = ours
end

-- DirectInput key names <-> our key ids (extended << 8 | set 1 scan code).
local KEYS = {
	ESCAPE = 0x01, ["1"] = 0x02, ["2"] = 0x03, ["3"] = 0x04, ["4"] = 0x05, ["5"] = 0x06,
	["6"] = 0x07, ["7"] = 0x08, ["8"] = 0x09, ["9"] = 0x0A, ["0"] = 0x0B, MINUS = 0x0C,
	EQUALS = 0x0D, BACK = 0x0E, TAB = 0x0F, Q = 0x10, W = 0x11, E = 0x12, R = 0x13, T = 0x14,
	Y = 0x15, U = 0x16, I = 0x17, O = 0x18, P = 0x19, LBRACKET = 0x1A, RBRACKET = 0x1B,
	RETURN = 0x1C, A = 0x1E, S = 0x1F, D = 0x20, F = 0x21, G = 0x22, H = 0x23, J = 0x24,
	K = 0x25, L = 0x26, SEMICOLON = 0x27, APOSTROPHE = 0x28, GRAVE = 0x29, BACKSLASH = 0x2B,
	Z = 0x2C, X = 0x2D, C = 0x2E, V = 0x2F, B = 0x30, N = 0x31, M = 0x32, COMMA = 0x33,
	PERIOD = 0x34, SLASH = 0x35, MULTIPLY = 0x37, LMENU = 0x38, SPACE = 0x39, CAPITAL = 0x3A,
	F1 = 0x3B, F2 = 0x3C, F3 = 0x3D, F4 = 0x3E, F5 = 0x3F, F6 = 0x40, F7 = 0x41, F8 = 0x42,
	F9 = 0x43, F10 = 0x44, PAUSE = 0x45, SCROLL = 0x46, NUMPAD7 = 0x47, NUMPAD8 = 0x48,
	NUMPAD9 = 0x49, SUBTRACT = 0x4A, NUMPAD4 = 0x4B, NUMPAD5 = 0x4C, NUMPAD6 = 0x4D, ADD = 0x4E,
	NUMPAD1 = 0x4F, NUMPAD2 = 0x50, NUMPAD3 = 0x51, NUMPAD0 = 0x52, DECIMAL = 0x53, OEM_102 = 0x56,
	F11 = 0x57, F12 = 0x58, NUMPADENTER = 0x11C, DIVIDE = 0x135, SYSRQ = 0x137, RMENU = 0x138,
	NUMLOCK = 0x145, HOME = 0x147, UP = 0x148, PRIOR = 0x149, LEFT = 0x14B, RIGHT = 0x14D,
	END = 0x14F, DOWN = 0x150, NEXT = 0x151, INSERT = 0x152, DELETE = 0x153, LWIN = 0x15B,
	RWIN = 0x15C, APPS = 0x15D,
}
local KEY_NAMES = {}
for name, id in KEYS do
	KEY_NAMES[id] = name
end
-- The keys held, as the code's suffix spells them (either Shift or Ctrl key).
local SHIFT = { [0x2A] = true, [0x36] = true }
local CONTROL = { [0x1D] = true, [0x11D] = true }

local TAGS = { INPUT_ACTION_ = "action", INPUT_STATE_ = "state", INPUT_RANGE_ = "range" }

local function tag_of(id)
	for prefix, tag in TAGS do
		if string.sub(id, 1, #prefix) == prefix then
			return tag
		end
	end
	return nil
end

local function slash(path)
	path = string.gsub(path, "\\", "/")
	if path ~= "" and string.sub(path, -1) ~= "/" then
		path ..= "/"
	end
	return path
end

plugin.targets(function(ctx)
	local found, seen = {}, {}
	local function add(base, name)
		if seen[base] then
			return
		end
		seen[base] = true
		table.insert(found, {
			id = string.sub(base, 1, -2),
			name = name,
			-- GOG keeps the files right in the folder, Steam in one folder per account.
			files = { map = base .. "inputmap.xml", config = base .. "config.xml" },
			globs = { maps = base .. "*/inputmap.xml", configs = base .. "*/config.xml" },
		})
	end
	if ctx.env.documents ~= "" then
		add(slash(ctx.env.documents) .. "Egosoft/X4/", "X4: Foundations")
	end
	for _, root in ctx.env.roots do
		root = slash(root)
		if ctx.env.platform == "linux" then
			-- The native game in the home folder; the Windows one in a Proton prefix.
			add(root .. ".config/EgoSoft/X4/", "X4: Foundations")
			add(root .. "users/steamuser/Documents/Egosoft/X4/", "X4: Foundations (Proton)")
		end
	end
	return found
end)

-- The game's files do not tell its version: the newest list of controls is used.
plugin.version(function(_ctx)
	return nil
end)

-- ---- the files ---------------------------------------------------------------------------------

-- Each folder with an inputmap.xml: {map = key in ctx.files, config = key or nil}, by path.
local function folders(ctx)
	local list = {}
	local function config_of(dir)
		for key, f in ctx.files do
			if (key == "config" or string.sub(key, 1, 8) == "configs:")
				and f.text ~= nil
				and slash(string.match(f.path, "^(.*)[/\\]") or "") == dir
			then
				return key
			end
		end
		return nil
	end
	for key, f in ctx.files do
		if (key == "map" or string.sub(key, 1, 5) == "maps:") and f.text ~= nil then
			local dir = slash(string.match(f.path, "^(.*)[/\\]") or "")
			table.insert(list, { map = key, config = config_of(dir), path = f.path })
		end
	end
	table.sort(list, function(a, b)
		return a.path < b.path
	end)
	return list
end

-- A GUID's hex digits, lower case: "{42CEC570-0A3F-11F0-800B-444553540000}" and X4's
-- "{42cec570-0a3f-11f0-80-0b-44-45-53-54-00-00}" both give the same.
local function guid_digits(text)
	return string.lower(string.gsub(text or "", "[^%x]", ""))
end

-- config.xml: slot (1-8) of each pinned device, by GUID digits.
local function slots(text)
	local pinned = {}
	local doc = text and xml.parse(text)
	local root = doc and xml.root(doc)
	local joysticks = root and xml.find(root, "joysticks")
	for _, j in (if joysticks then xml.elements(joysticks, "joystick") else {}) do
		local num = tonumber(j.attrs.num or "")
		local disabled = j.attrs.disabled == "1" or j.attrs.disabled == "true"
		local current = j.attrs.current ~= "0" and j.attrs.current ~= "false"
		local guid = guid_digits(type(j.children[1]) == "string" and j.children[1] or "")
		if num and num >= 0 and num < 8 and current and not disabled and #guid == 32 then
			if pinned[guid] == nil then
				pinned[guid] = num + 1
			end
		end
	end
	return pinned
end

-- Our virtual joysticks the game has pinned to slots: {output, game_device = "joy<slot>"}.
local function found_links(ctx, folder)
	local links = {}
	local config = folder and folder.config and ctx.files[folder.config]
	local pinned = slots(config and config.text)
	for _, o in ctx.ours do
		local slot = o.instance ~= "" and pinned[guid_digits(o.instance)]
		if slot then
			table.insert(links, { output = o.output, game_device = "joy" .. slot })
		end
	end
	return links
end

-- Device links: those given, else those found. Returns output -> slot and back.
local function links_of(ctx, folder)
	local given = ctx.devices or {}
	if #given == 0 then
		given = found_links(ctx, folder)
	end
	local slot_of, output_of = {}, {}
	for _, l in given do
		local slot = tonumber(string.match(l.game_device, "^joy(%d)$") or "")
		if slot and slot >= 1 and slot <= 8 then
			slot_of[l.output] = slot
			output_of[slot] = l.output
		end
	end
	return slot_of, output_of
end

-- ---- lines <-> outputs -------------------------------------------------------------------------

-- "INPUT_SOURCE_JOYBUTTONS_3" -> "JOYBUTTONS", 3.
local function joy_source(source)
	local kind, slot = string.match(source or "", "^INPUT_SOURCE_(JOY%u+)_(%d)$")
	if kind == nil then
		kind = string.match(source or "", "^INPUT_SOURCE_(JOY%u+)$")
		slot = kind and "1"
	end
	return kind, tonumber(slot)
end

-- A line's input as our output id; nil for what the profile cannot send.
local function to_output(line, output_of)
	local a = line.attrs
	if a.sgn ~= nil then
		return nil
	end
	local source, code = a.source or "", a.code or ""
	if source == "INPUT_SOURCE_KEYBOARD" then
		local name, held = string.match(code, "^INPUT_KEYCODE_(.-)_(SHIFT_CONTROL)$")
		if name == nil then
			name, held = string.match(code, "^INPUT_KEYCODE_(.-)_(SHIFT)$")
		end
		if name == nil then
			name, held = string.match(code, "^INPUT_KEYCODE_(.-)_(CONTROL)$")
		end
		if name == nil then
			name = string.match(code, "^INPUT_KEYCODE_(.+)$")
		end
		local id = name and KEYS[name]
		if id == nil then
			return nil
		end
		local parts = {}
		if held == "CONTROL" or held == "SHIFT_CONTROL" then
			table.insert(parts, "key:0x1d")
		end
		if held == "SHIFT" or held == "SHIFT_CONTROL" then
			table.insert(parts, "key:0x2a")
		end
		table.insert(parts, string.format("key:0x%x", id))
		return table.concat(parts, "+")
	end
	local kind, slot = joy_source(source)
	local device = slot and output_of[slot]
	if device == nil then
		return nil
	end
	if kind == "JOYBUTTONS" then
		local name = string.match(code, "^INPUT_XBUTTON_(.+)$") or ""
		if HAT_BACK[name] then
			return device .. ":hat:1:" .. HAT_BACK[name]
		end
		local n = NAMED_BACK[name] or tonumber(name)
		if n and n >= 1 and n <= MAX_BUTTON then
			return device .. ":button:" .. n
		end
	elseif kind == "JOYAXES" then
		local axis = AXES_BACK[string.match(code, "^INPUT_JOYAXIS_(.+)$") or ""]
		if axis then
			return device .. ":axis:" .. axis
		end
	end
	return nil
end

local function is_keys(output)
	return string.match(output, "^key:") ~= nil or string.match(output, "^mouse:") ~= nil
end

-- Our output id -> {source, code}; nil for what the game cannot bind to a control of `tag`.
local function to_input(output, tag, slot_of)
	if is_keys(output) then
		if tag == "range" then
			return nil
		end
		local key, shift, control = nil, false, false
		for _, part in string.split(output, "+") do
			local id = tonumber(string.match(part, "^key:0x(%x+)$") or "", 16)
			if id == nil then
				return nil
			elseif SHIFT[id] then
				shift = true
			elseif CONTROL[id] then
				control = true
			elseif key == nil and KEY_NAMES[id] then
				key = KEY_NAMES[id]
			else
				return nil
			end
		end
		if key == nil then
			return nil
		end
		local suffix = if shift and control then "_SHIFT_CONTROL" elseif shift then "_SHIFT" elseif control then "_CONTROL" else ""
		return { source = "INPUT_SOURCE_KEYBOARD", code = "INPUT_KEYCODE_" .. key .. suffix }
	end
	local device, rest = string.match(output, "^(vjoy:%d+):(.+)$")
	local slot = device and slot_of[device]
	if slot == nil then
		return nil
	end
	local suffix = if slot == 1 then "" else "_" .. slot
	local button = tonumber(string.match(rest, "^button:(%d+)$") or "")
	local hat, dir = string.match(rest, "^hat:(%d+):(%a+)$")
	local axis = string.match(rest, "^axis:(%w+)$")
	if axis and AXES[axis] then
		-- Only a range takes a whole axis.
		return if tag == "range" then { source = "INPUT_SOURCE_JOYAXES" .. suffix, code = "INPUT_JOYAXIS_" .. AXES[axis] } else nil
	end
	if tag == "range" then
		return nil
	end
	if button and button >= 1 and button <= MAX_BUTTON then
		local name = NAMED_BUTTONS[button] or tostring(button)
		return { source = "INPUT_SOURCE_JOYBUTTONS" .. suffix, code = "INPUT_XBUTTON_" .. name }
	end
	if hat == "1" and HAT[dir] then
		return { source = "INPUT_SOURCE_JOYBUTTONS" .. suffix, code = "INPUT_XBUTTON_" .. HAT[dir] }
	end
	return nil
end

-- How a line's input reads in the list of changes: "joystick 2 XBUTTON_14", "keyboard TAB_SHIFT".
local function describe(source, code)
	local kind, slot = joy_source(source)
	local who = if kind then "joystick " .. slot else string.lower(string.match(source, "^INPUT_SOURCE_(.+)$") or source)
	return who .. " " .. (string.match(code, "^INPUT_[%u]+_(.+)$") or code)
end

-- The controls' lines in the map: id -> {lines}, for the catalog's controls.
local function lines_of(root, catalog)
	local known = {}
	for _, a in catalog.actions do
		known[a.id] = true
	end
	local by_id = {}
	for _, el in xml.elements(root) do
		local id = el.attrs.id
		if id and known[id] and tag_of(id) == el.tag then
			by_id[id] = by_id[id] or {}
			table.insert(by_id[id], el)
		end
	end
	return by_id
end

-- ---- read --------------------------------------------------------------------------------------

plugin.read(function(ctx)
	local list = folders(ctx)
	local folder = list[1]
	local links = found_links(ctx, folder)
	if folder == nil then
		return { devices = links }
	end
	local _, output_of = links_of(ctx, folder)
	local root = xml.root(xml.parse(ctx.files[folder.map].text))
	local binds, seen = {}, {}
	for id, lines in lines_of(root, ctx.catalog) do
		for _, line in lines do
			local output = to_output(line, output_of)
			if output and not seen[id .. "|" .. output] then
				seen[id .. "|" .. output] = true
				table.insert(binds, { action = id, output = output })
			end
		end
	end
	return { devices = links, binds = binds }
end)

-- ---- write -------------------------------------------------------------------------------------

-- Puts `child` right after `anchor` in `parent`, indented like it.
local function insert_after(parent, anchor, child)
	local list = parent.children
	for i, c in list do
		if c == anchor then
			local indent = list[i - 1]
			if type(indent) ~= "string" or not string.match(indent, "^%s*$") then
				indent = "\n  "
			end
			table.insert(list, i + 1, child)
			table.insert(list, i + 1, indent)
			return
		end
	end
	xml.append(parent, child)
end

-- The last element of `parent` that `keep` accepts.
local function last_of(parent, keep)
	local found = nil
	for _, el in xml.elements(parent) do
		if keep(el) then
			found = el
		end
	end
	return found
end

-- One inputmap.xml with the profile's lines; nil when it stays as it is.
local function write_map(ctx, folder, keyed)
	local slot_of, output_of = links_of(ctx, folder)
	local ours = {}
	for _, slot in slot_of do
		ours[slot] = true
	end
	local wanted, skipped, unlinked = {}, {}, false
	for _, b in ctx.binds do
		local tag = tag_of(b.action)
		local input = tag and to_input(b.output, tag, slot_of)
		if input then
			wanted[b.action] = wanted[b.action] or {}
			table.insert(wanted[b.action], input)
		else
			local device = string.match(b.output, "^(vjoy:%d+)")
			if device and slot_of[device] == nil then
				unlinked = true
			else
				skipped[b.action .. ": " .. b.output] = true
			end
		end
	end
	if unlinked then
		return nil, true
	end
	for text in skipped do
		plugin.log(text .. " cannot be bound in the game (an axis on a button control, or a button on an axis control?)")
	end

	-- A line is the profile's when it is on one of our slots, or is a key of a control held here,
	-- and the profile could have written it.
	local function mine(id, line)
		local source = line.attrs.source or ""
		if source == "INPUT_SOURCE_KEYBOARD" then
			return keyed[id] == true and to_output(line, {}) ~= nil
		end
		local _, slot = joy_source(source)
		return slot ~= nil and ours[slot] == true and to_output(line, output_of) ~= nil
	end

	local doc = xml.parse(ctx.files[folder.map].text)
	local root = xml.root(doc)
	local by_id = lines_of(root, ctx.catalog)
	local ids = {}
	for id in wanted do
		ids[id] = true
	end
	for id, lines in by_id do
		for _, line in lines do
			if mine(id, line) then
				ids[id] = true
			end
		end
	end
	local sorted = {}
	for id in ids do
		table.insert(sorted, id)
	end
	table.sort(sorted)

	local changes = {}
	for _, id in sorted do
		local before, after = {}, {}
		local want = wanted[id] or {}
		local placed = {}
		for _, line in by_id[id] or {} do
			if mine(id, line) then
				local text = describe(line.attrs.source, line.attrs.code)
				table.insert(before, text)
				local keep = false
				for n, w in want do
					if not placed[n] and w.source == line.attrs.source and w.code == line.attrs.code then
						placed[n], keep = true, true
						break
					end
				end
				if keep then
					table.insert(after, text)
				else
					xml.remove(root, line)
				end
			end
		end
		local tag = tag_of(id)
		for n, w in want do
			if not placed[n] then
				placed[n] = true
				local line = xml.element(tag, { id = id, source = w.source, code = w.code }, { "id", "source", "code" })
				-- After the control's other lines, else at the end of its section.
				local anchor = last_of(root, function(el)
					return el.attrs.id == id
				end) or last_of(root, function(el)
					return el.tag == tag
				end)
				if anchor then
					insert_after(root, anchor, line)
				else
					xml.append(root, line)
				end
				table.insert(after, describe(w.source, w.code))
			end
		end
		table.sort(before)
		table.sort(after)
		local from, to = table.concat(before, ", "), table.concat(after, ", ")
		if from ~= to then
			if from == "" then
				table.insert(changes, { kind = "added", action = id, to = to })
			elseif to == "" then
				table.insert(changes, { kind = "removed", action = id, from = from })
			else
				table.insert(changes, { kind = "changed", action = id, from = from, to = to })
			end
		end
	end
	return xml.write(doc), false, changes
end

plugin.write(function(ctx)
	local list = folders(ctx)
	if #list == 0 then
		error("X4 has not written its bindings yet: start the game once and leave it from the main menu.")
	end
	local keyed = {}
	for _, id in ctx.cleared or {} do
		keyed[id] = true
	end
	for _, b in ctx.binds do
		if is_keys(b.output) then
			keyed[b.action] = true
		end
	end
	local files, changes, written = {}, {}, 0
	for _, folder in list do
		local text, unlinked, folder_changes = write_map(ctx, folder, keyed)
		if text then
			files[folder.map] = text
			written += 1
			-- The same profile in every account: its changes once.
			if #changes == 0 then
				changes = folder_changes
			end
		elseif unlinked and #list == 1 then
			error("X4 does not know which slots the virtual joysticks have. Pick them once in the game (Settings > Controls > Manage controllers/joysticks), or type their slots (joy1 ... joy8) above the table.")
		end
	end
	if written == 0 then
		error("X4 does not know which slots the virtual joysticks have. Pick them once in the game (Settings > Controls > Manage controllers/joysticks), or type their slots (joy1 ... joy8) above the table.")
	end
	return { files = files, changes = changes }
end)
