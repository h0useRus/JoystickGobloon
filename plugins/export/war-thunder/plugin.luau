-- War Thunder: the game keeps the controls on its servers; the player saves them to a file in
-- Controls > the disk icon > Export and loads a file back with Import. The plugin works on such
-- exported files (.blk, the game's DataBlock text) in Documents/My Games/WarThunder, the game's
-- folder or a folder the player picks:
--
--   controls{
--     hotkeys{
--       ID_GEAR{                       one binding of an action; the block repeated = another
--         keyboardKey:i=34             binding; several inputs in one block = held together
--       }
--       ID_FIRE_PRIMARY{
--         joyButton:i=20               a joystick button, numbered across all devices
--       }
--       ID_FLAPS{}                     no binding (the game's default stays off)
--     }
--     axes{
--       elevator{
--         axisId:i=7                   a joystick axis, numbered across all devices; -1 = none
--         inverse:b=yes
--       }
--     }
--     deviceMapping{
--       joystick{ devId:t="1234:BEAD" name:t="vJoy Device" buttonsOffset:i=20 buttonsCount:i=36
--                 axesOffset:i=6 axesCount:i=10 connected:b=yes }
--     }
--   }
--
-- A device's buttons come first, then four per hat (up, right, down, left); its axes in
-- DirectInput order (X, Y, Z, Rx, Ry, Rz, sliders), then two per hat. Keys are DirectInput key
-- codes, mouse buttons 0 (left) to 4. Bindings with a joystick button held together with
-- something else, and settings like activationType, are kept as they are, never written.

local DIRS = { up = 0, right = 1, down = 2, left = 3 }
local DIR_NAMES = { [0] = "up", "right", "down", "left" }
local MOUSE = { [0] = "left", "right", "middle", "back", "forward" }
local MOUSE_BACK = {}
for n, name in MOUSE do
	MOUSE_BACK[name] = n
end
local INPUTS = { keyboardKey = true, mouseButton = true, joyButton = true }

local function slash(path)
	path = string.gsub(path, "\\", "/")
	if path ~= "" and string.sub(path, -1) ~= "/" then
		path ..= "/"
	end
	return path
end

plugin.targets(function(ctx)
	local env = ctx.env
	local globs, text, n = {}, {}, 0
	local seen = {}
	local function add(dir)
		dir = slash(dir)
		if dir == "/" or seen[dir] then
			return
		end
		seen[dir] = true
		n += 1
		local key = "presets" .. n
		globs[key] = dir .. "*.blk"
		text[key] = "controls{"
	end
	if env.documents ~= "" then
		local docs = slash(env.documents)
		add(docs)
		add(docs .. "My Games/WarThunder")
		add(docs .. "My Games/WarThunder/Saves")
	end
	local lad = slash(env.local_app_data or "")
	if lad ~= "" and lad ~= "/" then
		-- The game's own launcher installs it here.
		add(lad .. "WarThunder")
	end
	for _, root in env.roots do
		root = slash(root)
		add(root)
		add(root .. "War Thunder")
		add(root .. "WarThunder")
		add(root .. "SteamLibrary/steamapps/common/War Thunder")
		add(root .. "Program Files (x86)/Steam/steamapps/common/War Thunder")
		add(root .. "Program Files/Steam/steamapps/common/War Thunder")
		if env.platform == "linux" then
			add(root .. ".local/share/Steam/steamapps/common/War Thunder")
			add(root .. ".steam/steam/steamapps/common/War Thunder")
			add(root .. "WarThunder")
			add(root .. "users/steamuser/Documents/My Games/WarThunder")
		end
	end
	return { { id = "war-thunder", name = "War Thunder", globs = globs, glob_text = text } }
end)

-- The exported files do not tell the game version: the newest list of controls is used.
plugin.version(function(_ctx)
	return nil
end)

-- ---- DataBlock text ----------------------------------------------------------------------------

-- Blocks are {name, items}; items are blocks or {key, type, value} (value as written, quotes
-- included for text). Parses what the game writes; anything else ends the block it is in.
local function parse(text)
	local root = { name = "", items = {} }
	local stack = { root }
	for raw in string.gmatch((string.gsub(text or "", "\r", "")), "[^\n]+") do
		local line = string.match(raw, "^%s*(.-)%s*$")
		local top = stack[#stack]
		if line == "" then
			continue
		elseif line == "}" then
			if #stack > 1 then
				table.remove(stack)
			end
		else
			local name, rest = string.match(line, "^([%w_%.%-]+)%s*{(.*)$")
			if name then
				local block = { name = name, items = {} }
				table.insert(top.items, block)
				if string.match(rest, "^%s*}%s*$") == nil then
					table.insert(stack, block)
				end
			else
				local key, kind, value = string.match(line, "^([%w_%.%-]+):(%a+)%s*=%s*(.*)$")
				if key then
					table.insert(top.items, { key = key, type = kind, value = value })
				end
			end
		end
	end
	return root
end

-- The game's layout: two spaces per level, a blank line around a block with something in it
-- (after it, and before it after a value).
local function emit(block, depth, out)
	local pad = string.rep("  ", depth)
	for i, item in block.items do
		if item.name then
			if #item.items == 0 then
				table.insert(out, pad .. item.name .. "{}")
			else
				local prev = block.items[i - 1]
				if prev and prev.name == nil then
					table.insert(out, "")
				end
				table.insert(out, pad .. item.name .. "{")
				emit(item, depth + 1, out)
				table.insert(out, pad .. "}")
				if i < #block.items then
					table.insert(out, "")
				end
			end
		else
			table.insert(out, pad .. item.key .. ":" .. item.type .. "=" .. item.value)
		end
	end
end

local function write_blk(root, newline)
	local out = {}
	emit(root, 0, out)
	return table.concat(out, newline) .. newline
end

local function child(block, name)
	for _, item in block and block.items or {} do
		if item.name == name then
			return item
		end
	end
	return nil
end

local function param(block, key)
	for _, item in block.items do
		if item.key == key then
			return item
		end
	end
	return nil
end

local function number_of(item)
	return item and tonumber(item.value)
end

local function text_of(item)
	return item and string.match(item.value, '^"(.*)"$') or (item and item.value)
end

-- ---- devices -----------------------------------------------------------------------------------

-- deviceMapping's joysticks, in order: {devId, name, connected, buttons, button_count, axes,
-- axis_count}.
local function devices_of(controls)
	local list = {}
	for _, j in (child(controls, "deviceMapping") or { items = {} }).items do
		if j.name == "joystick" then
			table.insert(list, {
				dev_id = string.upper(text_of(param(j, "devId")) or ""),
				name = text_of(param(j, "name")) or "",
				connected = (param(j, "connected") or { value = "yes" }).value ~= "no",
				buttons = number_of(param(j, "buttonsOffset")) or 0,
				button_count = number_of(param(j, "buttonsCount")) or 0,
				axes = number_of(param(j, "axesOffset")) or 0,
				axis_count = number_of(param(j, "axesCount")) or 0,
			})
		end
	end
	return list
end

-- "{BEAD1234-0000-...}" (product, vendor) -> "1234:BEAD".
local function dev_id_of(guid)
	local hex = string.upper(string.gsub(guid or "", "[^%x]", ""))
	if #hex < 8 then
		return ""
	end
	return string.sub(hex, 5, 8) .. ":" .. string.sub(hex, 1, 4)
end

-- Our devices the file knows: {output, game_device = "joystick <n>"}, n counting deviceMapping's
-- joysticks from 1. A name only one of them has decides; else devices with the same ids go in
-- order, the connected ones first.
local function found_links(ctx, list)
	local links, taken = {}, {}
	local by_id = {}
	local ours = {}
	for _, o in ctx.ours do
		table.insert(ours, o)
	end
	table.sort(ours, function(a, b)
		return a.output < b.output
	end)
	for _, o in ours do
		local id = dev_id_of(o.guid)
		local matches = {}
		for n, d in list do
			if not taken[n] and d.dev_id == id and d.name == o.name then
				table.insert(matches, n)
			end
		end
		if #matches == 1 then
			taken[matches[1]] = true
			table.insert(links, { output = o.output, game_device = "joystick " .. matches[1] })
		else
			by_id[id] = by_id[id] or {}
			table.insert(by_id[id], o)
		end
	end
	for id, group in by_id do
		local free = {}
		for _, connected in { true, false } do
			for n, d in list do
				if not taken[n] and d.dev_id == id and d.connected == connected then
					table.insert(free, n)
				end
			end
		end
		for i, o in group do
			if free[i] then
				taken[free[i]] = true
				table.insert(links, { output = o.output, game_device = "joystick " .. free[i] })
			end
		end
	end
	table.sort(links, function(a, b)
		return a.output < b.output
	end)
	return links
end

-- Our devices with their place in the file: output -> {device, ours}, the links given or else
-- those found.
local function placed(ctx, list)
	local given = ctx.devices or {}
	if #given == 0 then
		given = found_links(ctx, list)
	end
	local ours = {}
	for _, o in ctx.ours do
		ours[o.output] = o
	end
	local out = {}
	for _, l in given do
		local n = tonumber(string.match(l.game_device, "^%s*joystick%s*(%d+)%s*$") or "")
		if n and list[n] and ours[l.output] then
			out[l.output] = { device = list[n], ours = ours[l.output] }
		end
	end
	return out
end

-- Whether the device's hats come as four buttons each after its buttons.
local function hat_buttons(p)
	return p.ours.hats > 0 and p.device.button_count - p.ours.buttons == 4 * p.ours.hats
end

-- ---- inputs <-> outputs ------------------------------------------------------------------------

-- DirectInput key codes <-> our key ids (extended << 8 | set 1 scan code); NumLock and Pause
-- swap places (Windows marks NumLock extended, DirectInput marks Pause).
local function key_id(dik)
	if dik == 0x45 then
		return 0x145
	elseif dik == 0xC5 then
		return 0x45
	elseif dik >= 0x80 then
		return 0x100 + dik - 0x80
	end
	return dik
end

local function dik_of(id)
	if id == 0x145 then
		return 0x45
	elseif id == 0x45 then
		return 0xC5
	elseif id >= 0x100 then
		return 0x80 + id % 0x100
	end
	return id
end

-- A global joystick button -> our output on one of our devices, or nil.
local function button_output(n, places)
	for output, p in places do
		local d = p.device
		local i = n - d.buttons
		if i >= 0 and i < d.button_count then
			if i < p.ours.buttons then
				return output .. ":button:" .. (i + 1)
			end
			if hat_buttons(p) then
				local h = i - p.ours.buttons
				return output .. ":hat:" .. (h // 4 + 1) .. ":" .. DIR_NAMES[h % 4]
			end
			return nil
		end
	end
	return nil
end

-- Whether a global joystick button belongs to one of our devices.
local function on_ours(n, places)
	for _, p in places do
		if n >= p.device.buttons and n < p.device.buttons + p.device.button_count then
			return true
		end
	end
	return false
end

-- A hotkey block's inputs as our output, or nil for what the profile cannot send.
local function block_output(block, places)
	local keys, joy = {}, {}
	for _, item in block.items do
		if item.key == "keyboardKey" then
			local n = number_of(item)
			if n == nil then
				return nil
			end
			table.insert(keys, string.format("key:0x%x", key_id(n)))
		elseif item.key == "mouseButton" then
			local name = MOUSE[number_of(item) or -1]
			if name == nil then
				return nil
			end
			table.insert(keys, "mouse:" .. name)
		elseif item.key == "joyButton" then
			table.insert(joy, number_of(item))
		elseif item.name then
			return nil
		end
	end
	if #joy == 1 and #keys == 0 then
		return joy[1] and button_output(joy[1], places)
	elseif #joy == 0 and #keys > 0 then
		return table.concat(keys, "+")
	end
	return nil
end

local function is_keys(output)
	return string.match(output, "^key:") ~= nil or string.match(output, "^mouse:") ~= nil
end

-- Our output -> the items of a hotkey block, or nil.
local function block_items(output, places)
	if is_keys(output) then
		local items = {}
		for _, part in string.split(output, "+") do
			local id = tonumber(string.match(part, "^key:0x(%x+)$") or "", 16)
			local mouse = MOUSE_BACK[string.match(part, "^mouse:(%a+)$") or ""]
			if id then
				table.insert(items, { key = "keyboardKey", type = "i", value = tostring(dik_of(id)) })
			elseif mouse then
				table.insert(items, { key = "mouseButton", type = "i", value = tostring(mouse) })
			else
				return nil
			end
		end
		return items
	end
	local device, rest = string.match(output, "^(vjoy:%d+):(.+)$")
	local p = device and places[device]
	if p == nil then
		return nil
	end
	local button = tonumber(string.match(rest, "^button:(%d+)$") or "")
	local hat, dir = string.match(rest, "^hat:(%d+):(%a+)$")
	local i = nil
	if button and button >= 1 and button <= p.ours.buttons then
		i = button - 1
	elseif hat and DIRS[dir] and hat_buttons(p) and tonumber(hat) <= p.ours.hats then
		i = p.ours.buttons + 4 * (tonumber(hat) - 1) + DIRS[dir]
	end
	if i == nil or i >= p.device.button_count then
		return nil
	end
	return { { key = "joyButton", type = "i", value = tostring(p.device.buttons + i) } }
end

-- A global axis -> our axis output, or nil.
local function axis_output(n, places)
	for output, p in places do
		local i = n - p.device.axes
		if i >= 0 and i < p.device.axis_count then
			local name = p.ours.axes[i + 1]
			return name and (output .. ":axis:" .. name)
		end
	end
	return nil
end

local function on_our_axis(n, places)
	for _, p in places do
		if n >= p.device.axes and n < p.device.axes + p.device.axis_count then
			return true
		end
	end
	return false
end

-- Our axis output -> the global axis number, or nil.
local function axis_number(output, places)
	local device, name = string.match(output, "^(vjoy:%d+):axis:(%w+)$")
	local p = device and places[device]
	if p == nil then
		return nil
	end
	for i, a in p.ours.axes do
		if a == name and i <= p.device.axis_count then
			return p.device.axes + i - 1
		end
	end
	return nil
end

-- How a block's inputs read in the list of changes.
local function describe(block, list)
	local parts = {}
	for _, item in block.items do
		if item.key == "keyboardKey" then
			table.insert(parts, string.format("key 0x%x", key_id(number_of(item) or 0)))
		elseif item.key == "mouseButton" then
			table.insert(parts, "mouse " .. (MOUSE[number_of(item) or -1] or item.value))
		elseif item.key == "joyButton" then
			local n = number_of(item) or 0
			local text = "button " .. n
			for _, d in list do
				if n >= d.buttons and n < d.buttons + d.button_count then
					text = d.name .. " button " .. (n - d.buttons + 1)
				end
			end
			table.insert(parts, text)
		end
	end
	return table.concat(parts, " + ")
end

-- ---- the files ---------------------------------------------------------------------------------

-- The exported controls files found: {key, path}, sorted by path.
local function presets(ctx)
	local list = {}
	for key, f in ctx.files do
		if string.match(key, "^presets%d+:") and f.text ~= nil and string.find(f.text, "hotkeys{", 1, true) then
			table.insert(list, { key = key, path = f.path })
		end
	end
	table.sort(list, function(a, b)
		return a.path < b.path
	end)
	return list
end

local function known(catalog)
	local set = {}
	for _, a in catalog.actions do
		set[a.id] = a.kind or "button"
	end
	return set
end

-- ---- read --------------------------------------------------------------------------------------

plugin.read(function(ctx)
	local list = presets(ctx)
	if #list == 0 then
		return { devices = {} }
	end
	local controls = child(parse(ctx.files[list[1].key].text), "controls")
	local devices = devices_of(controls)
	local links = found_links(ctx, devices)
	local places = placed(ctx, devices)
	local kinds = known(ctx.catalog)
	local binds, seen = {}, {}
	local function add(action, output, invert)
		if output and not seen[action .. "|" .. output] then
			seen[action .. "|" .. output] = true
			table.insert(binds, { action = action, output = output, invert = invert or nil })
		end
	end
	for _, block in (child(controls, "hotkeys") or { items = {} }).items do
		if block.name and kinds[block.name] == "button" then
			add(block.name, block_output(block, places))
		end
	end
	for _, block in (child(controls, "axes") or { items = {} }).items do
		local n = block.name and number_of(param(block, "axisId"))
		if n and n >= 0 and kinds[block.name] == "axis" then
			local inverse = param(block, "inverse")
			add(block.name, axis_output(n, places), inverse ~= nil and inverse.value == "yes")
		end
	end
	return { devices = links, binds = binds }
end)

-- ---- write -------------------------------------------------------------------------------------

local function same_items(a, b)
	if #a ~= #b then
		return false
	end
	for i, item in a do
		if item.key ~= b[i].key or item.value ~= b[i].value then
			return false
		end
	end
	return true
end

-- The block's inputs only, for comparing.
local function inputs_of(block)
	local out = {}
	for _, item in block.items do
		if INPUTS[item.key] then
			table.insert(out, item)
		end
	end
	return out
end

-- Where a new block named `name` goes in `parent`: after its own blocks, else in name order.
local function insert_block(parent, block)
	local at = nil
	for i, item in parent.items do
		if item.name == block.name then
			at = i + 1
		end
	end
	if at == nil then
		at = #parent.items + 1
		for i, item in parent.items do
			if item.name and item.name > block.name then
				at = i
				break
			end
		end
	end
	table.insert(parent.items, at, block)
end

local function change(changes, id, before, after)
	table.sort(before)
	table.sort(after)
	local from, to = table.concat(before, ", "), table.concat(after, ", ")
	if from == to then
		return
	elseif from == "" then
		table.insert(changes, { kind = "added", action = id, to = to })
	elseif to == "" then
		table.insert(changes, { kind = "removed", action = id, from = from })
	else
		table.insert(changes, { kind = "changed", action = id, from = from, to = to })
	end
end

-- One file with the profile's bindings: its text and changes; nil text when a virtual joystick of
-- the profile is not in the file.
local function write_file(ctx, text, keyed)
	local newline = if string.find(text, "\r\n", 1, true) then "\r\n" else "\n"
	local root = parse(text)
	local controls = child(root, "controls")
	if controls == nil then
		return nil, "no controls"
	end
	local devices = devices_of(controls)
	local places = placed(ctx, devices)
	local kinds = known(ctx.catalog)
	local hotkeys = child(controls, "hotkeys")
	if hotkeys == nil then
		hotkeys = { name = "hotkeys", items = {} }
		table.insert(controls.items, hotkeys)
	end
	local axes = child(controls, "axes")
	if axes == nil then
		axes = { name = "axes", items = {} }
		table.insert(controls.items, axes)
	end

	local buttons, axis_binds, skipped, unlinked = {}, {}, {}, false
	for _, b in ctx.binds do
		local kind = kinds[b.action]
		local device = string.match(b.output, "^(vjoy:%d+)")
		if device and places[device] == nil then
			unlinked = true
		elseif kind == "axis" then
			local n = axis_number(b.output, places)
			if n then
				axis_binds[b.action] = { number = n, invert = b.invert }
			else
				skipped[b.action .. ": " .. b.output] = true
			end
		elseif kind == "button" then
			local items = block_items(b.output, places)
			if items then
				buttons[b.action] = buttons[b.action] or {}
				table.insert(buttons[b.action], items)
			else
				skipped[b.action .. ": " .. b.output] = true
			end
		end
	end
	if unlinked then
		return nil, "unlinked"
	end
	for line in skipped do
		plugin.log(line .. " cannot be bound in the game (an axis on a button, a hat the file has no buttons for?)")
	end

	-- A block is the profile's when the profile could have written it: one button of one of our
	-- devices, or keys of an action whose keys the profile holds.
	local function mine(id, block)
		local ins = inputs_of(block)
		if #ins == 0 or #ins ~= #block.items then
			return false
		end
		if #ins == 1 and ins[1].key == "joyButton" then
			return on_ours(number_of(ins[1]) or -1, places)
		end
		for _, item in ins do
			if item.key == "joyButton" then
				return false
			end
		end
		return keyed[id] == true and block_output(block, places) ~= nil
	end

	local changes = {}
	local ids = {}
	for id in buttons do
		ids[id] = true
	end
	for _, block in hotkeys.items do
		if block.name and kinds[block.name] == "button" and mine(block.name, block) then
			ids[block.name] = true
		end
	end
	local sorted = {}
	for id in ids do
		table.insert(sorted, id)
	end
	table.sort(sorted)
	for _, id in sorted do
		local want = buttons[id] or {}
		local placed_n, before, after = {}, {}, {}
		local had, kept_any = false, false
		for i = #hotkeys.items, 1, -1 do
			local block = hotkeys.items[i]
			if block.name == id then
				had = true
				if #block.items == 0 then
					-- An empty slot (the game writes them next to bindings too): left as it is.
					kept_any = true
				elseif mine(id, block) then
					local text = describe(block, devices)
					table.insert(before, text)
					local keep = false
					for n, w in want do
						if not placed_n[n] and same_items(w, inputs_of(block)) then
							placed_n[n], keep = true, true
							break
						end
					end
					if keep then
						table.insert(after, text)
						kept_any = true
					else
						table.remove(hotkeys.items, i)
					end
				else
					kept_any = true
				end
			end
		end
		for n, w in want do
			if not placed_n[n] then
				local block = { name = id, items = w }
				insert_block(hotkeys, block)
				table.insert(after, describe(block, devices))
				kept_any = true
			end
		end
		if had and not kept_any then
			insert_block(hotkeys, { name = id, items = {} })
		end
		change(changes, id, before, after)
	end

	-- Axes: the profile's axis, or none where one of our axes was.
	local axis_ids = {}
	for id in axis_binds do
		axis_ids[id] = true
	end
	for _, block in axes.items do
		local n = block.name and number_of(param(block, "axisId"))
		if n and kinds[block.name] == "axis" and on_our_axis(n, places) then
			axis_ids[block.name] = true
		end
	end
	local axis_sorted = {}
	for id in axis_ids do
		table.insert(axis_sorted, id)
	end
	table.sort(axis_sorted)
	for _, id in axis_sorted do
		local block = child(axes, id)
		if block == nil then
			block = { name = id, items = {} }
			insert_block(axes, block)
		end
		local item = param(block, "axisId")
		local old = number_of(item) or -1
		local before = if old >= 0 and on_our_axis(old, places) then { (axis_output(old, places) or ("axis " .. old)) } else {}
		local want = axis_binds[id]
		local after = {}
		if want then
			if item == nil then
				item = { key = "axisId", type = "i", value = "-1" }
				table.insert(block.items, 1, item)
			end
			item.value = tostring(want.number)
			table.insert(after, axis_output(want.number, places) or ("axis " .. want.number))
			local inverse = param(block, "inverse")
			if want.invert then
				if inverse then
					inverse.value = "yes"
				else
					table.insert(block.items, { key = "inverse", type = "b", value = "yes" })
				end
				after[1] ..= " (inverted)"
			elseif inverse and inverse.value == "yes" then
				inverse.value = "no"
			end
		elseif item and old >= 0 and on_our_axis(old, places) then
			item.value = "-1"
		end
		change(changes, id, before, after)
	end
	return write_blk(root, newline), nil, changes
end

plugin.write(function(ctx)
	local list = presets(ctx)
	if #list == 0 then
		error("No exported War Thunder controls were found. Save them from the game first (Controls > the disk icon > Export) into Documents/My Games/WarThunder, then export again; load the file back in the game with Import.")
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
	for _, preset in list do
		local text, why, file_changes = write_file(ctx, ctx.files[preset.key].text, keyed)
		if text then
			files[preset.key] = text
			written += 1
			-- The same profile in every file: its changes once.
			if #changes == 0 then
				changes = file_changes
			end
		elseif why == "unlinked" then
			plugin.log(preset.path .. ": the game had not seen every virtual joystick of the profile when it was saved")
		end
	end
	if written == 0 then
		error("War Thunder had not seen the virtual joysticks when the controls were exported. Start the profile, open the game's Controls with the virtual joysticks connected, export the controls again, or type which joystick of the file each one is (joystick 1, joystick 2 ...) above the table.")
	end
	return { files = files, changes = changes }
end)
