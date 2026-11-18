-- IL-2 Sturmovik: Great Battles: the key mapping in <install>/data/input/global.actions, one
-- line per input of an action:
--
--   action,<pad to 50>input,<pad to 100>invert| // the game's English text
--
-- Inputs: joy<N>_b<button, from 0>, joy<N>_axis_<x|y|z|w|s|t|p|q>, joy<N>_pov<hat, from 0>_<angle>,
-- key_<DirectInput name>, mouse_b<0|1|2>; "modifier+input" for keys held together, "first/second"
-- for the two buttons of an axis. An action without input has an empty input.
-- joy<N> is the device's number in devices.txt ("N,%22<guid>%22,<name>|"). The game compiles
-- current.map from these files at every start; it is never written here.

local INSTALLS = {
	"",
	"IL-2 Sturmovik Great Battles/",
	"IL-2 Sturmovik Battle of Stalingrad/",
	"Games/IL-2 Sturmovik Great Battles/",
	"Program Files/1C Game Studios/IL-2 Sturmovik Great Battles/",
	"Program Files (x86)/1C Game Studios/IL-2 Sturmovik Great Battles/",
	"Program Files (x86)/Steam/steamapps/common/IL-2 Sturmovik Battle of Stalingrad/",
	"Steam/steamapps/common/IL-2 Sturmovik Battle of Stalingrad/",
	"SteamLibrary/steamapps/common/IL-2 Sturmovik Battle of Stalingrad/",
	"Games/SteamLibrary/steamapps/common/IL-2 Sturmovik Battle of Stalingrad/",
	"steamapps/common/IL-2 Sturmovik Battle of Stalingrad/",
}

-- Our axis names (output ids) <-> the game's.
local AXES = { x = "x", y = "y", z = "z", rx = "w", ry = "s", rz = "t", slider = "p", dial = "q" }
local AXES_BACK = {}
for ours, game in AXES do
	AXES_BACK[game] = ours
end

-- Hat directions <-> the game's angles; diagonals are not bound one by one.
local ANGLES = { up = "0", right = "90", down = "180", left = "270" }
local ANGLES_BACK = {}
for ours, game in ANGLES do
	ANGLES_BACK[game] = ours
end

-- The game's key names (DirectInput's) <-> our key ids (extended << 8 | set 1 scan code).
local KEYS = {
	escape = 0x01, ["1"] = 0x02, ["2"] = 0x03, ["3"] = 0x04, ["4"] = 0x05, ["5"] = 0x06,
	["6"] = 0x07, ["7"] = 0x08, ["8"] = 0x09, ["9"] = 0x0A, ["0"] = 0x0B, minus = 0x0C,
	equals = 0x0D, back = 0x0E, tab = 0x0F, q = 0x10, w = 0x11, e = 0x12, r = 0x13,
	t = 0x14, y = 0x15, u = 0x16, i = 0x17, o = 0x18, p = 0x19, lbracket = 0x1A,
	rbracket = 0x1B, ["return"] = 0x1C, lcontrol = 0x1D, a = 0x1E, s = 0x1F, d = 0x20, f = 0x21,
	g = 0x22, h = 0x23, j = 0x24, k = 0x25, l = 0x26, semicolon = 0x27, apostrophe = 0x28,
	grave = 0x29, lshift = 0x2A, backslash = 0x2B, z = 0x2C, x = 0x2D, c = 0x2E, v = 0x2F,
	b = 0x30, n = 0x31, m = 0x32, comma = 0x33, period = 0x34, slash = 0x35, rshift = 0x36,
	multiply = 0x37, lmenu = 0x38, space = 0x39, capital = 0x3A, f1 = 0x3B, f2 = 0x3C,
	f3 = 0x3D, f4 = 0x3E, f5 = 0x3F, f6 = 0x40, f7 = 0x41, f8 = 0x42, f9 = 0x43, f10 = 0x44,
	scroll = 0x46, numpad7 = 0x47, numpad8 = 0x48, numpad9 = 0x49, subtract = 0x4A,
	numpad4 = 0x4B, numpad5 = 0x4C, numpad6 = 0x4D, add = 0x4E, numpad1 = 0x4F, numpad2 = 0x50,
	numpad3 = 0x51, numpad0 = 0x52, decimal = 0x53, f11 = 0x57, f12 = 0x58,
	numpadenter = 0x11C, rcontrol = 0x11D, divide = 0x135, sysrq = 0x137, rmenu = 0x138,
	numlock = 0x145, home = 0x147, up = 0x148, prior = 0x149, left = 0x14B, right = 0x14D,
	["end"] = 0x14F, down = 0x150, next = 0x151, insert = 0x152, delete = 0x153,
	lwin = 0x15B, rwin = 0x15C,
}
local KEY_NAMES = {}
for name, id in KEYS do
	KEY_NAMES[id] = name
end

-- Our mouse button names <-> the game's.
local MOUSE = { left = "mouse_b0", right = "mouse_b1", middle = "mouse_b2" }
local MOUSE_BACK = {}
for ours, game in MOUSE do
	MOUSE_BACK[game] = ours
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
	for _, root in ctx.env.roots do
		root = slash(root)
		for _, install in INSTALLS do
			local base = root .. install
			if not seen[base] then
				seen[base] = true
				local input = base .. "data/input"
				table.insert(found, {
					id = string.sub(base, 1, -2),
					name = "Great Battles",
					files = { actions = input .. "/global.actions", devices = input .. "/devices.txt" },
				})
			end
		end
	end
	return found
end)

-- The game's files do not tell its version: the newest list of actions is used.
plugin.version(function(_ctx)
	return nil
end)

-- ---- the files ---------------------------------------------------------------------------------

local function unescape(s)
	return (string.gsub(s, "%%(%x%x)", function(hex)
		return string.char(tonumber(hex, 16))
	end))
end

-- devices.txt: the game's joysticks in order, { js = "joy3", name = "vJoy Device" }.
local function game_devices(text)
	local list = {}
	for line in string.gmatch(text or "", "[^\r\n]+") do
		local id, name = string.match(line, "^%s*(%d+),[^,]*,(.-)|%s*$")
		if id then
			table.insert(list, { js = "joy" .. id, name = unescape(name) })
		end
	end
	return list
end

-- Our virtual joysticks as the game numbers them: by name, in the order of both lists (every
-- vJoy device has the same name).
local function found_links(ctx)
	local f = ctx.files.devices
	local links, used = {}, {}
	for _, d in game_devices(f and f.text) do
		for _, ours in ctx.ours do
			if not used[ours.output] and string.lower(ours.name) == string.lower(d.name) then
				used[ours.output] = true
				table.insert(links, { output = ours.output, game_device = d.js })
				break
			end
		end
	end
	return links
end

-- Device links: those given, else those found in devices.txt.
local function links_of(ctx)
	local given = ctx.devices or {}
	if #given == 0 then
		given = found_links(ctx)
	end
	local js_of, output_of = {}, {}
	for _, l in given do
		if string.match(l.game_device, "^joy%d+$") then
			js_of[l.output] = l.game_device
			output_of[l.game_device] = l.output
		end
	end
	return js_of, output_of
end

-- The key mapping file as lines: { raw } for comments and the header, and for a mapping line
-- also action, command, invert ("0"/"1") and comment.
local function parse(text)
	local eol = if string.find(text, "\r\n", 1, true) then "\r\n" else "\n"
	local lines = {}
	for _, raw in string.split(text, "\n") do
		raw = string.gsub(raw, "\r$", "")
		local line = { raw = raw }
		if string.sub(raw, 1, 2) ~= "//" and string.sub(raw, 1, 1) ~= "&" then
			local action, command, invert, rest = string.match(raw, "^%s*([%w_]+)%s*,%s*(.-)%s*,%s*(%d)%s*|%s*(.*)$")
			if action then
				line.action, line.command, line.invert = action, command, invert
				line.comment = string.match(rest, "^//%s*(.-)%s*$") or ""
			end
		end
		table.insert(lines, line)
	end
	return lines, eol
end

local function format(action, command, invert, comment)
	local line = string.format("%-50s%-50s%s|", action .. ",", command .. ",", invert)
	return if comment ~= "" then line .. " // " .. comment else line
end

-- ---- outputs <-> inputs ------------------------------------------------------------------------

-- One side of a command ("key_lcontrol+key_r", "joy0_b5") -> our output id; nil for what the
-- profile cannot send or the game binds on a device that is not ours.
local function to_output(side, output_of)
	local js, rest = string.match(side, "^(joy%d+)_([%w_]+)$")
	if js then
		local device = output_of[js]
		if device == nil then
			return nil
		end
		local button = string.match(rest, "^b(%d+)$")
		if button then
			return device .. ":button:" .. (tonumber(button) + 1)
		end
		local axis = string.match(rest, "^axis_(%a)$")
		if axis and AXES_BACK[axis] then
			return device .. ":axis:" .. AXES_BACK[axis]
		end
		local hat, angle = string.match(rest, "^pov(%d+)_(%d+)$")
		if hat and ANGLES_BACK[angle] then
			return device .. ":hat:" .. (tonumber(hat) + 1) .. ":" .. ANGLES_BACK[angle]
		end
		return nil
	end
	local parts = {}
	for _, name in string.split(side, "+") do
		local key = string.match(name, "^key_([%w]+)$")
		if key and KEYS[key] then
			table.insert(parts, string.format("key:0x%x", KEYS[key]))
		elseif MOUSE_BACK[name] then
			table.insert(parts, "mouse:" .. MOUSE_BACK[name])
		else
			return nil
		end
	end
	return if #parts > 0 then table.concat(parts, "+") else nil
end

local function is_keys(output)
	return string.match(output, "^key:") ~= nil or string.match(output, "^mouse:") ~= nil
end

-- Our output id -> the game's input; nil for what the game cannot bind.
local function to_input(output, js_of)
	if is_keys(output) then
		local names = {}
		for _, part in string.split(output, "+") do
			local key = string.match(part, "^key:0x(%x+)$")
			local button = string.match(part, "^mouse:([%a%-]+)$")
			local name = nil
			if key then
				name = KEY_NAMES[tonumber(key, 16)]
				name = name and "key_" .. name
			elseif button then
				name = MOUSE[button]
			end
			if name == nil then
				return nil
			end
			table.insert(names, name)
		end
		return table.concat(names, "+")
	end
	local device, rest = string.match(output, "^(vjoy:%d+):(.+)$")
	local js = device and js_of[device]
	if js == nil then
		return nil
	end
	local button = string.match(rest, "^button:(%d+)$")
	if button then
		return js .. "_b" .. (tonumber(button) - 1)
	end
	local hat, dir = string.match(rest, "^hat:(%d+):(%a+)$")
	if hat and ANGLES[dir] then
		return js .. "_pov" .. (tonumber(hat) - 1) .. "_" .. ANGLES[dir]
	end
	local axis = string.match(rest, "^axis:(%w+)$")
	if axis and AXES[axis] then
		return js .. "_axis_" .. AXES[axis]
	end
	return nil
end

-- The action a catalog id binds and which button of an axis: "rpc_yaw_trim:inc" -> rpc_yaw_trim, 1.
local function split_id(id)
	local base, half = string.match(id, "^(.-):(%a+)$")
	if half == "inc" then
		return base, 1
	elseif half == "dec" then
		return base, 2
	end
	return id, nil
end

-- ---- read --------------------------------------------------------------------------------------

plugin.read(function(ctx)
	local f = ctx.files.actions
	local links = found_links(ctx)
	if f == nil or f.text == nil then
		return { devices = links }
	end
	local output_of = {}
	for _, l in links do
		output_of[l.game_device] = l.output
	end
	local binds = {}
	for _, line in parse(f.text) do
		-- The mouse in the game's own menus is nothing to bind.
		if line.action and line.command ~= "" and string.sub(line.action, 1, 10) ~= "gui_mouse_" then
			local sides = string.split(line.command, "/")
			if #sides == 1 then
				local output = to_output(sides[1], output_of)
				if output then
					table.insert(binds, { action = line.action, output = output, invert = line.invert == "1" })
				end
			elseif #sides == 2 then
				for n, half in { "inc", "dec" } do
					local output = to_output(sides[n], output_of)
					if output then
						table.insert(binds, { action = line.action .. ":" .. half, output = output })
					end
				end
			end
		end
	end
	return { devices = links, binds = binds }
end)

-- ---- write -------------------------------------------------------------------------------------

local function sorted_keys(t)
	local keys = {}
	for k in t do
		table.insert(keys, k)
	end
	table.sort(keys)
	return keys
end

plugin.write(function(ctx)
	local f = ctx.files.actions
	if f == nil or f.text == nil then
		error("The game has not written its key mapping yet: open Settings > Key mapping in the game and press Accept once.")
	end
	local js_of = links_of(ctx)
	local ours = {}
	for _, js in js_of do
		ours[js] = true
	end

	-- What each action gets: joystick inputs, and keyboard and mouse inputs of the actions the
	-- profile holds them for (`keyed`; the other actions keep the game's).
	local keyed = {}
	for _, id in ctx.cleared or {} do
		keyed[(split_id(id))] = true
	end
	local singles, halves, unlinked = {}, {}, false
	for _, b in ctx.binds do
		local action, half = split_id(b.action)
		if is_keys(b.output) then
			keyed[action] = true
		end
		local input = to_input(b.output, js_of)
		if input == nil then
			if string.match(b.output, "^vjoy:") and js_of[string.match(b.output, "^(vjoy:%d+)")] == nil then
				unlinked = true
			end
		elseif half then
			halves[action] = halves[action] or { {}, {} }
			table.insert(halves[action][half], input)
		else
			singles[action] = singles[action] or {}
			table.insert(singles[action], { command = input, invert = b.invert })
		end
	end
	if unlinked then
		error("The game does not list the virtual joysticks yet. Start it once with them connected, or type their numbers in the game (joy0, joy1 ...) above the table.")
	end
	local wanted = singles
	for action, sides in halves do
		-- An axis takes its two buttons on one line; a button without its partner is not written.
		for n = 1, math.min(#sides[1], #sides[2]) do
			wanted[action] = wanted[action] or {}
			table.insert(wanted[action], { command = sides[1][n] .. "/" .. sides[2][n] })
		end
		if #sides[1] ~= #sides[2] then
			plugin.log(action .. ": an axis from buttons needs both its (+) and (-) button; the single one is not written")
		end
	end

	-- A line is the profile's when every input on it is on our joysticks, or is a key or mouse
	-- button of an action held here. Other lines (other devices, mixed, mouse motion) stay.
	local function mine(action, command)
		local joy, keys = 0, 0
		for _, side in string.split(command, "/") do
			for _, token in string.split(side, "+") do
				local js = string.match(token, "^(joy%d+)_")
				if js and ours[js] then
					joy += 1
				elseif not js and to_output(token, {}) then
					keys += 1
				else
					return false
				end
			end
		end
		return (joy > 0 and keys == 0) or (keys > 0 and joy == 0 and keyed[action] == true)
	end

	local lines, eol = parse(f.text)
	-- Each action's lines, at the place of its first one.
	local of, order = {}, {}
	for n, line in lines do
		if line.action then
			if of[line.action] == nil then
				of[line.action] = {}
				table.insert(order, line.action)
			end
			table.insert(of[line.action], n)
		end
	end

	local changes, replaced, dropped = {}, {}, {}
	local function change(action, before, after)
		table.sort(before)
		table.sort(after)
		local from, to = table.concat(before, ", "), table.concat(after, ", ")
		if from == to then
			return
		end
		if from == "" then
			table.insert(changes, { kind = "added", action = action, to = to })
		elseif to == "" then
			table.insert(changes, { kind = "removed", action = action, from = from })
		else
			table.insert(changes, { kind = "changed", action = action, from = from, to = to })
		end
	end

	for _, action in order do
		local kept, before, inverted, comment = {}, {}, {}, ""
		for _, n in of[action] do
			local line = lines[n]
			if comment == "" then
				comment = line.comment
			end
			if line.command ~= "" then
				if mine(action, line.command) then
					table.insert(before, line.command)
					inverted[line.command] = line.invert
				else
					table.insert(kept, line)
				end
			end
		end
		local new, after, same = wanted[action] or {}, {}, true
		for _, w in new do
			table.insert(after, w.command)
			-- The game's invert stays unless the profile inverts the binding.
			if inverted[w.command] == nil or (w.invert and inverted[w.command] ~= "1") then
				same = false
			end
		end
		if same and #before == #after then
			continue
		end
		change(action, before, table.clone(after))
		-- The action's lines again: the game's other inputs first, then ours; the text on the first.
		local out = {}
		for _, line in kept do
			table.insert(out, format(action, line.command, line.invert, if #out == 0 then comment else ""))
		end
		for _, w in new do
			local invert = if w.invert then "1" else (inverted[w.command] or "0")
			table.insert(out, format(action, w.command, invert, if #out == 0 then comment else ""))
		end
		if #out == 0 then
			table.insert(out, format(action, "", "0", comment))
		end
		for i, n in of[action] do
			if i == 1 then
				replaced[n] = out
			else
				dropped[n] = true
			end
		end
	end

	local text = {}
	for n, line in lines do
		if replaced[n] then
			for _, l in replaced[n] do
				table.insert(text, l)
			end
		elseif not dropped[n] then
			table.insert(text, line.raw)
		end
	end
	-- Actions the file does not have yet go to its end, before a last empty line.
	local tail = {}
	for _, action in sorted_keys(wanted) do
		if of[action] == nil then
			local a = nil
			for _, c in ctx.catalog.actions do
				if c.id == action then
					a = c
					break
				end
			end
			local after = {}
			for i, w in wanted[action] do
				table.insert(tail, format(action, w.command, if w.invert then "1" else "0", if i == 1 and a then a.name else ""))
				table.insert(after, w.command)
			end
			change(action, {}, after)
		end
	end
	if #tail > 0 then
		local last = if text[#text] == "" then table.remove(text) else nil
		for _, l in tail do
			table.insert(text, l)
		end
		if last then
			table.insert(text, last)
		end
	end

	local result = table.concat(text, eol)
	return { files = { actions = result }, changes = changes }
end)
