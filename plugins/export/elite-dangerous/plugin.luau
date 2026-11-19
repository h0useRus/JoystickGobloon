-- Elite Dangerous: the bindings presets in %LOCALAPPDATA%/Frontier Developments/Elite Dangerous/
-- Options/Bindings, <Preset>.<major>.<minor>.binds, written by the game when the player saves in
-- Options > Controls. StartPreset.4.start names the active preset of General, Ship, SRV and On foot
-- (one line each); the Ship line's preset is the file read and written. Built-in presets live in
-- the install and are never written.
--
-- Every element is one game action:
--
--   <YawAxisRaw>                                  an axis: one slot
--     <Binding Device="1234BEAD" Key="Joy_XAxis" />
--     <Inverted Value="0" />
--     <Deadzone Value="0.00000000" />
--   </YawAxisRaw>
--   <LandingGearToggle>                           a button: two slots
--     <Primary Device="1234BEAD" DeviceIndex="1" Key="Joy_5" />
--     <Secondary Device="Keyboard" Key="Key_L">
--       <Modifier Device="Keyboard" Key="Key_LeftShift" />
--     </Secondary>
--   </LandingGearToggle>
--
-- Device = "Keyboard", "Mouse", "{NoDevice}" for an empty slot, or a controller's USB vendor and
-- product id as 8 hex digits (vJoy: 1234BEAD). Devices with the same ids are told apart by
-- DeviceIndex (0 when missing). Joystick keys: Joy_<button>, Joy_POV<hat><Up|Right|Down|Left>,
-- Joy_<X|Y|Z|RX|RY|RZ|U|V>Axis; Pos_/Neg_ halves of axes are kept as they are, never written.

local BINDINGS = "Frontier Developments/Elite Dangerous/Options/Bindings/"

-- Our axis names (output ids, DirectInput slots 1-8) <-> the game's keys.
local AXES = {
	x = "Joy_XAxis",
	y = "Joy_YAxis",
	z = "Joy_ZAxis",
	rx = "Joy_RXAxis",
	ry = "Joy_RYAxis",
	rz = "Joy_RZAxis",
	slider = "Joy_UAxis",
	dial = "Joy_VAxis",
}
local AXES_BACK = {}
for ours, game in AXES do
	AXES_BACK[game] = ours
end

-- Hat directions; the game binds no diagonals.
local HAT = { up = "Up", right = "Right", down = "Down", left = "Left" }
local HAT_BACK = {}
for ours, game in HAT do
	HAT_BACK[game] = ours
end

-- The game's key names (US layout) <-> our key ids (extended << 8 | set 1 scan code).
local KEYS = {
	Key_Escape = 0x01, Key_1 = 0x02, Key_2 = 0x03, Key_3 = 0x04, Key_4 = 0x05, Key_5 = 0x06,
	Key_6 = 0x07, Key_7 = 0x08, Key_8 = 0x09, Key_9 = 0x0A, Key_0 = 0x0B, Key_Minus = 0x0C,
	Key_Equals = 0x0D, Key_Backspace = 0x0E, Key_Tab = 0x0F, Key_Q = 0x10, Key_W = 0x11,
	Key_E = 0x12, Key_R = 0x13, Key_T = 0x14, Key_Y = 0x15, Key_U = 0x16, Key_I = 0x17,
	Key_O = 0x18, Key_P = 0x19, Key_LeftBracket = 0x1A, Key_RightBracket = 0x1B, Key_Enter = 0x1C,
	Key_LeftControl = 0x1D, Key_A = 0x1E, Key_S = 0x1F, Key_D = 0x20, Key_F = 0x21, Key_G = 0x22,
	Key_H = 0x23, Key_J = 0x24, Key_K = 0x25, Key_L = 0x26, Key_SemiColon = 0x27,
	Key_Apostrophe = 0x28, Key_Grave = 0x29, Key_LeftShift = 0x2A, Key_BackSlash = 0x2B,
	Key_Z = 0x2C, Key_X = 0x2D, Key_C = 0x2E, Key_V = 0x2F, Key_B = 0x30, Key_N = 0x31,
	Key_M = 0x32, Key_Comma = 0x33, Key_Period = 0x34, Key_Slash = 0x35, Key_RightShift = 0x36,
	Key_Numpad_Multiply = 0x37, Key_LeftAlt = 0x38, Key_Space = 0x39, Key_CapsLock = 0x3A,
	Key_F1 = 0x3B, Key_F2 = 0x3C, Key_F3 = 0x3D, Key_F4 = 0x3E, Key_F5 = 0x3F, Key_F6 = 0x40,
	Key_F7 = 0x41, Key_F8 = 0x42, Key_F9 = 0x43, Key_F10 = 0x44, Key_Pause = 0x45,
	Key_ScrollLock = 0x46, Key_Numpad_7 = 0x47, Key_Numpad_8 = 0x48, Key_Numpad_9 = 0x49,
	Key_Numpad_Subtract = 0x4A, Key_Numpad_4 = 0x4B, Key_Numpad_5 = 0x4C, Key_Numpad_6 = 0x4D,
	Key_Numpad_Add = 0x4E, Key_Numpad_1 = 0x4F, Key_Numpad_2 = 0x50, Key_Numpad_3 = 0x51,
	Key_Numpad_0 = 0x52, Key_Numpad_Decimal = 0x53, Key_OEM_102 = 0x56, Key_F11 = 0x57,
	Key_F12 = 0x58, Key_F13 = 0x64, Key_F14 = 0x65, Key_F15 = 0x66, Key_F16 = 0x67, Key_F17 = 0x68,
	Key_F18 = 0x69, Key_F19 = 0x6A, Key_F20 = 0x6B, Key_F21 = 0x6C, Key_F22 = 0x6D, Key_F23 = 0x6E,
	Key_F24 = 0x76, Key_Numpad_Enter = 0x11C, Key_RightControl = 0x11D, Key_Numpad_Divide = 0x135,
	Key_SYSRQ = 0x137, Key_RightAlt = 0x138, Key_NumLock = 0x145, Key_Home = 0x147,
	Key_UpArrow = 0x148, Key_PageUp = 0x149, Key_LeftArrow = 0x14B, Key_RightArrow = 0x14D,
	Key_End = 0x14F, Key_DownArrow = 0x150, Key_PageDown = 0x151, Key_Insert = 0x152,
	Key_Delete = 0x153, Key_LeftWin = 0x15B, Key_RightWin = 0x15C, Key_Apps = 0x15D,
}
local KEY_NAMES = {}
for name, id in KEYS do
	KEY_NAMES[id] = name
end

-- Our mouse button names <-> the game's.
local MOUSE = { left = "Mouse_1", right = "Mouse_2", middle = "Mouse_3", back = "Mouse_4", forward = "Mouse_5" }
local MOUSE_BACK = {}
for ours, game in MOUSE do
	MOUSE_BACK[game] = ours
end

local NO_DEVICE = "{NoDevice}"

local function slash(path)
	path = string.gsub(path, "\\", "/")
	if path ~= "" and string.sub(path, -1) ~= "/" then
		path ..= "/"
	end
	return path
end

plugin.targets(function(ctx)
	local found, seen = {}, {}
	local function add(dir, name)
		if seen[dir] then
			return
		end
		seen[dir] = true
		table.insert(found, {
			id = string.sub(dir, 1, -2),
			name = name,
			files = { start = dir .. "StartPreset.4.start" },
			globs = { binds = dir .. "*.binds" },
		})
	end
	if ctx.env.local_app_data ~= "" then
		add(slash(ctx.env.local_app_data) .. BINDINGS, "Elite Dangerous")
	end
	-- Linux: the game runs in a Proton prefix; its drive_c folders are among the roots.
	for _, root in ctx.env.roots do
		add(slash(root) .. "users/steamuser/AppData/Local/" .. BINDINGS, "Elite Dangerous (Proton)")
	end
	return found
end)

-- ---- the files ---------------------------------------------------------------------------------

-- The presets StartPreset.4.start names: General, Ship, SRV, On foot.
local function start_presets(ctx)
	local list = {}
	local f = ctx.files.start
	for line in string.gmatch((f and f.text) or "", "[^\r\n]+") do
		line = string.match(line, "^%s*(.-)%s*$")
		if line ~= "" then
			table.insert(list, line)
		end
	end
	return list
end

-- The active Ship preset's file: its key in ctx.files and its minor version (the newest 4.x
-- file of that preset). Also returns the preset's name.
local function active_file(ctx)
	local presets = start_presets(ctx)
	local wanted = presets[2] or presets[1] or "Custom"
	local best, minor = nil, -1
	for key, f in ctx.files do
		if string.sub(key, 1, 6) == "binds:" and f.text ~= nil then
			local name = string.match(f.path, "([^/\\]+)%.binds$") or ""
			local preset, major, m = string.match(name, "^(.*)%.(%d+)%.(%d+)$")
			if preset == wanted and major == "4" and tonumber(m) > minor then
				best, minor = key, tonumber(m)
			end
		end
	end
	return best, minor, wanted
end

plugin.version(function(ctx)
	local key, minor = active_file(ctx)
	return if key then "4." .. minor else nil
end)

-- ---- devices -----------------------------------------------------------------------------------

-- "{BEAD1234-0000-0000-0000-504944564944}" (DirectInput product GUID: product, then vendor) ->
-- "1234BEAD" (the game's: vendor, then product).
local function usb_id(guid)
	local product, vendor = string.match(guid or "", "^{?(%x%x%x%x)(%x%x%x%x)%-")
	return if product then string.upper(vendor .. product) else nil
end

-- Our virtual joysticks as the game names them: the USB id, with ":<DeviceIndex>" for the
-- second and later device with the same id (in the order of our devices).
local function found_links(ctx)
	local count, links = {}, {}
	for _, o in ctx.ours do
		local id = string.match(o.output, "^vjoy:") and usb_id(o.guid)
		if id then
			local n = count[id] or 0
			count[id] = n + 1
			table.insert(links, { output = o.output, game_device = if n == 0 then id else id .. ":" .. n })
		end
	end
	return links
end

-- Device links: those given, else those found. Returns output -> game device and back.
local function links_of(ctx)
	local given = ctx.devices or {}
	if #given == 0 then
		given = found_links(ctx)
	end
	local game_of, output_of = {}, {}
	for _, l in given do
		local id = string.upper(l.game_device)
		if string.match(id, "^%x%x%x%x%x%x%x%x$") or string.match(id, "^%x%x%x%x%x%x%x%x:%d+$") then
			-- ":0" is the first device.
			id = string.gsub(id, ":0$", "")
			game_of[l.output] = id
			output_of[id] = l.output
		end
	end
	return game_of, output_of
end

-- A slot's device as a game device id: "1234BEAD" or "1234BEAD:1".
local function slot_device(slot)
	local device = slot.attrs.Device or NO_DEVICE
	local index = tonumber(slot.attrs.DeviceIndex or "0") or 0
	return if index > 0 then device .. ":" .. index else device
end

-- ---- slots <-> outputs -------------------------------------------------------------------------

local function is_empty(slot)
	return (slot.attrs.Device or NO_DEVICE) == NO_DEVICE or (slot.attrs.Key or "") == ""
end

-- A slot's input as our output id; nil for what the profile cannot send (half axes, joystick
-- buttons with modifiers, devices that are not ours, keys of other layouts).
local function to_output(slot, output_of)
	if is_empty(slot) then
		return nil
	end
	local device, key = slot.attrs.Device, slot.attrs.Key
	local modifiers = xml.elements(slot, "Modifier")
	if device == "Keyboard" then
		local parts = {}
		for _, m in modifiers do
			local id = m.attrs.Device == "Keyboard" and KEYS[m.attrs.Key]
			if not id then
				return nil
			end
			table.insert(parts, string.format("key:0x%x", id))
		end
		if KEYS[key] == nil then
			return nil
		end
		table.insert(parts, string.format("key:0x%x", KEYS[key]))
		return table.concat(parts, "+")
	end
	if #modifiers > 0 then
		return nil
	end
	if device == "Mouse" then
		return if MOUSE_BACK[key] then "mouse:" .. MOUSE_BACK[key] else nil
	end
	local output = output_of[string.upper(slot_device(slot))]
	if output == nil then
		return nil
	end
	local button = string.match(key, "^Joy_(%d+)$")
	if button then
		return output .. ":button:" .. button
	end
	local hat, dir = string.match(key, "^Joy_POV(%d)(%a+)$")
	if hat and HAT_BACK[dir] then
		return output .. ":hat:" .. hat .. ":" .. HAT_BACK[dir]
	end
	if AXES_BACK[key] then
		return output .. ":axis:" .. AXES_BACK[key]
	end
	return nil
end

local function is_keys(output)
	return string.match(output, "^key:") ~= nil or string.match(output, "^mouse:") ~= nil
end

-- Our output id -> the slot's content {device, index, key, modifiers = {key names}}; nil for
-- what the game cannot bind.
local function to_input(output, game_of)
	if string.match(output, "^mouse:") then
		local key = MOUSE[string.match(output, "^mouse:([%a%-]+)$") or ""]
		return if key and not string.find(output, "+", 1, true) then { device = "Mouse", index = 0, key = key, modifiers = {} } else nil
	end
	if is_keys(output) then
		local names = {}
		for _, part in string.split(output, "+") do
			local id = string.match(part, "^key:0x(%x+)$")
			local name = id and KEY_NAMES[tonumber(id, 16)]
			if name == nil then
				return nil
			end
			table.insert(names, name)
		end
		-- The last key is the one pressed; those before it are held.
		local key = table.remove(names)
		if #names > 3 then
			return nil
		end
		return { device = "Keyboard", index = 0, key = key, modifiers = names }
	end
	local device, rest = string.match(output, "^(vjoy:%d+):(.+)$")
	local game = device and game_of[device]
	if game == nil then
		return nil
	end
	local id, index = string.match(game, "^(%x+):(%d+)$")
	id = id or game
	index = tonumber(index or "0")
	local key = nil
	local button = string.match(rest, "^button:(%d+)$")
	local hat, dir = string.match(rest, "^hat:(%d+):(%a+)$")
	local axis = string.match(rest, "^axis:(%w+)$")
	if button then
		key = "Joy_" .. button
	elseif hat and HAT[dir] and tonumber(hat) <= 4 then
		key = "Joy_POV" .. hat .. HAT[dir]
	elseif axis and AXES[axis] then
		key = AXES[axis]
	end
	return if key then { device = id, index = index, key = key, modifiers = {} } else nil
end

-- How a slot's input reads in the list of changes.
local function describe(slot)
	local text = slot_device(slot) .. " " .. (slot.attrs.Key or "")
	for _, m in xml.elements(slot, "Modifier") do
		text = (m.attrs.Key or "") .. "+" .. text
	end
	return text
end

-- ---- read --------------------------------------------------------------------------------------

-- The game's element of each catalog action, with its kind ("axis" or "button").
local function elements(root, catalog)
	local out = {}
	for _, a in catalog.actions do
		local el = xml.find(root, a.id)
		if el then
			local kind = if xml.find(el, "Binding") then "axis" else "button"
			table.insert(out, { id = a.id, el = el, kind = kind })
		end
	end
	return out
end

local function slots_of(e)
	local list = {}
	for _, tag in (if e.kind == "axis" then { "Binding" } else { "Primary", "Secondary" }) do
		local s = xml.find(e.el, tag)
		if s then
			table.insert(list, s)
		end
	end
	return list
end

plugin.read(function(ctx)
	local links = found_links(ctx)
	local key = active_file(ctx)
	if key == nil then
		return { devices = links }
	end
	local _, output_of = links_of(ctx)
	local root = xml.root(xml.parse(ctx.files[key].text))
	local binds = {}
	for _, e in elements(root, ctx.catalog) do
		for _, slot in slots_of(e) do
			local output = to_output(slot, output_of)
			if output then
				local invert = nil
				if e.kind == "axis" then
					local inv = xml.find(e.el, "Inverted")
					invert = inv ~= nil and inv.attrs.Value == "1"
				end
				table.insert(binds, { action = e.id, output = output, invert = invert })
			end
		end
	end
	return { devices = links, binds = binds }
end)

-- ---- write -------------------------------------------------------------------------------------

-- Puts `input` into `slot` (keeping its Hold flag), or empties it when `input` is nil.
local function set_slot(slot, input)
	for _, m in xml.elements(slot, "Modifier") do
		xml.remove(slot, m)
	end
	if input == nil then
		for _, h in xml.elements(slot, "Hold") do
			xml.remove(slot, h)
		end
		slot.attrs.Device = NO_DEVICE
		slot.attrs.Key = ""
		slot.attrs.DeviceIndex = nil
		return
	end
	slot.attrs.Device = input.device
	slot.attrs.Key = input.key
	if input.index > 0 then
		slot.attrs.DeviceIndex = tostring(input.index)
	elseif slot.attrs.DeviceIndex ~= nil then
		slot.attrs.DeviceIndex = nil
	end
	for _, name in input.modifiers do
		xml.append(slot, xml.element("Modifier", { Device = "Keyboard", Key = name }, { "Device", "Key" }))
	end
end

plugin.write(function(ctx)
	local key, _, preset = active_file(ctx)
	if key == nil then
		error("Elite Dangerous uses the built-in preset \"" .. preset .. "\" for ship controls. Change one control in the game's Options > Controls and save: the game then writes your own preset, which can be filled.")
	end
	local game_of, output_of = links_of(ctx)
	local ours = {}
	for _, id in game_of do
		ours[id] = true
	end

	-- What each action gets: joystick inputs, and keyboard and mouse inputs of the actions the
	-- profile holds them for (`keyed`; the other actions keep the game's).
	local keyed = {}
	for _, id in ctx.cleared or {} do
		keyed[id] = true
	end
	local wanted, unlinked = {}, false
	for _, b in ctx.binds do
		if is_keys(b.output) then
			keyed[b.action] = true
		end
		local input = to_input(b.output, game_of)
		if input == nil then
			local device = string.match(b.output, "^(vjoy:%d+)")
			if device and game_of[device] == nil then
				unlinked = true
			else
				plugin.log(b.action .. ": " .. b.output .. " cannot be bound in the game")
			end
		else
			input.output = b.output
			input.invert = b.invert
			wanted[b.action] = wanted[b.action] or {}
			table.insert(wanted[b.action], input)
		end
	end
	if unlinked then
		error("A virtual joystick has no device id in the game yet. Type it above the table: 1234BEAD for the first vJoy device, 1234BEAD:1 for the second and so on.")
	end

	-- A slot is the profile's when it holds one of our joysticks (no modifiers, no half axis), or
	-- keys or a mouse button of an action held here.
	local function mine(action, slot)
		if is_empty(slot) then
			return false
		end
		local device = slot.attrs.Device
		if device == "Keyboard" or device == "Mouse" then
			return keyed[action] == true and to_output(slot, {}) ~= nil
		end
		return ours[string.upper(slot_device(slot))] == true and to_output(slot, output_of) ~= nil
	end

	local doc = xml.parse(ctx.files[key].text)
	local root = xml.root(doc)
	local changes, seen = {}, {}
	for _, e in elements(root, ctx.catalog) do
		seen[e.id] = true
		local want = wanted[e.id] or {}
		local before, after = {}, {}
		local free, placed = {}, {}
		for _, slot in slots_of(e) do
			local output = to_output(slot, output_of)
			local keep = false
			if output and mine(e.id, slot) then
				table.insert(before, describe(slot))
				for _, w in want do
					if w.output == output and not placed[w] then
						placed[w], keep = true, true
						table.insert(after, describe(slot))
						break
					end
				end
			end
			if not keep and (mine(e.id, slot) or is_empty(slot)) then
				table.insert(free, { slot = slot, ours = mine(e.id, slot) })
			end
		end
		if e.kind == "axis" and #want > 1 then
			plugin.log(e.id .. ": an axis takes one input in the game; the first one is written")
		end
		-- The game's invert stays while the same axis does, unless the profile inverts it.
		local kept_axis = e.kind == "axis" and want[1] ~= nil and placed[want[1]] == true
		for n, w in want do
			if not placed[w] and (e.kind ~= "axis" or n == 1) then
				local spot = table.remove(free, 1)
				if spot == nil then
					plugin.log(e.id .. ": both slots hold the game's other inputs; " .. w.output .. " is not written")
				else
					set_slot(spot.slot, w)
					spot.ours = false
					table.insert(after, describe(spot.slot))
				end
			end
		end
		-- Our inputs the profile no longer has.
		local cleared_axis = false
		for _, spot in free do
			if spot.ours then
				set_slot(spot.slot, nil)
				cleared_axis = e.kind == "axis"
			end
		end
		local inv = e.kind == "axis" and xml.find(e.el, "Inverted")
		if inv and want[1] and #after > 0 then
			if want[1].invert then
				inv.attrs.Value = "1"
			elseif not kept_axis then
				inv.attrs.Value = "0"
			end
		elseif inv and cleared_axis then
			inv.attrs.Value = "0"
		end
		table.sort(before)
		table.sort(after)
		local from, to = table.concat(before, ", "), table.concat(after, ", ")
		if from ~= to then
			if from == "" then
				table.insert(changes, { kind = "added", action = e.id, to = to })
			elseif to == "" then
				table.insert(changes, { kind = "removed", action = e.id, from = from })
			else
				table.insert(changes, { kind = "changed", action = e.id, from = from, to = to })
			end
		end
	end
	for action in wanted do
		if not seen[action] then
			plugin.log(action .. ": the preset has no such control (an older game version?); not written")
		end
	end
	return { files = { [key] = xml.write(doc) }, changes = changes }
end)
