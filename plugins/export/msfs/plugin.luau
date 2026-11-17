-- Microsoft Flight Simulator 2020 and 2024: a control profile per device, an XML file whose name
-- the sim makes up (Steam: <Steam>/userdata/<user>/<app>/remote/inputprofile_<hex>; Microsoft
-- Store: a cloud save blob under AppData/Local/Packages/<package>/SystemAppData/wgs):
--
--   <?xml version="1.0" encoding="UTF-8"?>
--   <Version Num="-1"/>
--   <FriendlyName PlatformAvailability="1" Locked="false">the profile's name</FriendlyName>
--   <Device DeviceName="vJoy Device" GUID="{instance GUID}" ProductID="..." ...>
--     <AircraftInfo CategoryName="AIRPLANE"/>          (2024: a category profile; none = general)
--     <Axes>...</Axes>                                 (sensitivity, dead zones: left alone)
--     <Context ContextName="SURFACES">
--       <Action ActionName="KEY_AXIS_ELEVATOR_SET" ValueEvent="0.000000" Delay="2.000000" Flag="4">
--         <Primary><KEY Information="Joystick L-Axis Y ">1042</KEY></Primary>
--       </Action>
--
-- KEY numbers: button N = N-1; hat 1 = 256 up, 257 right, 258 down, 259 left; axes = 256 * group
-- (2 slider, 3 rotation "R-Axis", 4 "L-Axis") + 16 * axis (X, Y, Z) + 2 (0 and 1 = its - and +
-- half). Flag bits: 1 or 2 = a button, 4 = an axis, 128 = the axis reversed. The 2024 sim lists
-- every action (unbound ones have no Primary), the 2020 sim only bound ones (and no ValueEvent or
-- Delay). An action has a Primary and a Secondary binding at most.
-- Steam profiles are updated in place. Store saves are only read: the 2024 sim gets files to
-- import (Controls > the device > Import), the 2020 sim has no way in.

local STEAM = {
	"Program Files (x86)/Steam/",
	"Program Files/Steam/",
	"Steam/",
	"Games/Steam/",
	".local/share/Steam/",
	".steam/steam/",
	".var/app/com.valvesoftware.Steam/data/Steam/",
}

local GAMES = {
	{ version = "2020", app = "1250410", package = "Microsoft.FlightSimulator_8wekyb3d8bbwe" },
	{
		version = "2024",
		app = "2537590",
		package = "Microsoft.Limitless_8wekyb3d8bbwe",
		import = "Flight Simulator 2024",
	},
}

-- Our axis names (output ids) <-> the sim's KEY numbers and names.
local AXES = {
	x = { 1026, "Joystick L-Axis X " },
	y = { 1042, "Joystick L-Axis Y " },
	z = { 1058, "Joystick L-Axis Z " },
	rx = { 770, "Joystick R-Axis X " },
	ry = { 786, "Joystick R-Axis Y " },
	rz = { 802, "Joystick R-Axis Z " },
	slider = { 514, "Joystick Slider X " },
	dial = { 530, "Joystick Slider Y " },
}
-- Hat 1's directions; the sim writes "JoystickPov Left" without a space.
local HAT = {
	up = { 256, "Joystick Pov Up" },
	right = { 257, "Joystick Pov Right" },
	down = { 258, "Joystick Pov Down" },
	left = { 259, "JoystickPov Left" },
}
local MAX_BUTTONS = 128

local BY_CODE = {}
for name, a in AXES do
	BY_CODE[a[1]] = "axis:" .. name
end
for dir, h in HAT do
	BY_CODE[h[1]] = "hat:1:" .. dir
end

local CATEGORY_NAMES = { [""] = "General", AIRPLANE = "Airplanes", HELICOPTER = "Helicopters" }

plugin.options({
	preset = {
		kind = "text",
		label = "Only the sim's control profile with this name (empty: every one of the device)",
		default = "",
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
	local function add(t)
		if not seen[t.id] then
			seen[t.id] = true
			table.insert(found, t)
		end
	end
	for _, root in ctx.env.roots do
		root = slash(root)
		for _, game in GAMES do
			for _, steam in STEAM do
				local base = root .. steam
				add({
					id = "steam" .. game.version .. ":" .. base,
					name = game.version .. " (Steam)",
					globs = { profile = base .. "userdata/*/" .. game.app .. "/remote/inputprofile_*" },
				})
			end
			if ctx.env.platform == "windows" then
				local t = {
					id = "store" .. game.version .. ":" .. root,
					name = game.version .. " (Microsoft Store)",
					globs = {
						profile = root .. "AppData/Local/Packages/" .. game.package .. "/SystemAppData/wgs/*/*/*",
					},
				}
				if game.import then
					t.dirs = { import = root .. "Documents/Joystick Gobloon/" .. game.import }
				end
				add(t)
			end
		end
	end
	return found
end)

-- "2020" or "2024", from the target the plugin made.
plugin.version(function(ctx)
	return string.match(ctx.target.id, "^%a+(%d+):")
end)

-- ---- profiles ----------------------------------------------------------------------------------

local function sorted_keys(t)
	local keys = {}
	for k in t do
		table.insert(keys, k)
	end
	table.sort(keys)
	return keys
end

local function text_of(el)
	local parts = {}
	for _, c in el.children or {} do
		if type(c) == "string" then
			table.insert(parts, c)
		end
	end
	return string.match(table.concat(parts), "^%s*(.-)%s*$")
end

local function lower(s)
	return string.lower(s or "")
end

-- One control profile: { key, doc, device (element), name, guid, category, preset, full }; nil for
-- files that are not one (other saves of the sim).
local function load(key, text)
	if not string.find(text, "<Device", 1, true) or not string.find(text, "<Context", 1, true) then
		return nil
	end
	local ok, doc = pcall(xml.parse, text)
	if not ok then
		return nil
	end
	local device = xml.find(doc, "Device")
	if device == nil then
		return nil
	end
	local friendly = xml.find(doc, "FriendlyName")
	local info = xml.find(device, "AircraftInfo")
	return {
		key = key,
		doc = doc,
		device = device,
		name = device.attrs.DeviceName or "",
		guid = device.attrs.GUID or "",
		category = info and info.attrs.CategoryName or "",
		preset = friendly and text_of(friendly) or "",
		-- The 2024 sim writes every action with ValueEvent and Delay.
		full = string.find(text, "ValueEvent=", 1, true) ~= nil,
	}
end

local function profiles_of(ctx)
	local list = {}
	for _, key in sorted_keys(ctx.files) do
		local f = ctx.files[key]
		if string.sub(key, 1, 8) == "profile:" and f.text then
			local p = load(key, f.text)
			if p then
				table.insert(list, p)
			end
		end
	end
	return list
end

-- Our virtual joysticks in the profiles: by name, each new GUID to the next of ours with that name.
local function found_links(profiles, ours)
	local links, used, taken = {}, {}, {}
	for _, p in profiles do
		local guid = lower(p.guid)
		if guid ~= "" and not taken[guid] then
			for _, o in ours do
				if not used[o.output] and lower(o.name) == lower(p.name) then
					used[o.output] = true
					taken[guid] = true
					table.insert(links, { output = o.output, game_device = p.guid })
					break
				end
			end
		end
	end
	return links
end

-- Device links: those given, else those found; output -> lowercase GUID and back.
local function links_of(ctx, profiles)
	local given = ctx.devices or {}
	if #given == 0 then
		given = found_links(profiles, ctx.ours)
	end
	local guid_of, output_of = {}, {}
	for _, l in given do
		local guid = lower(string.match(l.game_device, "^%s*(.-)%s*$"))
		if guid ~= "" then
			if string.sub(guid, 1, 1) ~= "{" then
				guid = "{" .. guid .. "}"
			end
			guid_of[l.output] = guid
			output_of[guid] = l.output
		end
	end
	return guid_of, output_of
end

-- ---- KEYs <-> outputs --------------------------------------------------------------------------

-- A KEY number -> the output on its device ("button:3", "axis:x", "hat:1:up"); nil for what the
-- profile cannot send (half axes, other hats).
local function decode(code)
	if code == nil then
		return nil
	end
	if code >= 0 and code < MAX_BUTTONS then
		return "button:" .. (code + 1)
	end
	return BY_CODE[code]
end

-- An output on a device -> the KEY number and its name; nil for what the sim cannot bind.
local function encode(what)
	local button = string.match(what, "^button:(%d+)$")
	if button then
		local n = tonumber(button)
		if n >= 1 and n <= MAX_BUTTONS then
			return n - 1, "Joystick Button " .. n
		end
		return nil
	end
	local axis = string.match(what, "^axis:(%w+)$")
	if axis and AXES[axis] then
		return AXES[axis][1], AXES[axis][2]
	end
	local dir = string.match(what, "^hat:1:(%a+)$")
	if dir and HAT[dir] then
		return HAT[dir][1], HAT[dir][2]
	end
	return nil
end

local function is_axis(what)
	return string.sub(what, 1, 5) == "axis:"
end

local function info_of(what)
	local _, info = encode(what)
	return info and string.match(info, "^(.-)%s*$") or what
end

-- "AIRPLANE:SURFACES/KEY_X" -> "AIRPLANE", "SURFACES", "KEY_X"; a general action has no category.
local function split_id(id)
	local category, context, action = string.match(id, "^([%w_]+):([%w_]+)/(.+)$")
	if category then
		return category, context, action
	end
	context, action = string.match(id, "^([%w_]+)/(.+)$")
	return "", context, action
end

local function id_of(p, context, action)
	if p.category ~= "" then
		return p.category .. ":" .. context .. "/" .. action
	end
	return context .. "/" .. action
end

-- The single KEY of a Primary or Secondary decoded; nil for none, several or what we cannot send.
local function slot_output(slot)
	local keys = xml.elements(slot, "KEY")
	if #keys ~= 1 then
		return nil
	end
	return decode(tonumber(text_of(keys[1])))
end

-- ---- read --------------------------------------------------------------------------------------

plugin.read(function(ctx)
	local profiles = profiles_of(ctx)
	local links = found_links(profiles, ctx.ours)
	local output_of = {}
	for _, l in links do
		output_of[lower(l.game_device)] = l.output
	end
	local binds, seen = {}, {}
	for _, p in profiles do
		local device = output_of[lower(p.guid)]
		if device then
			for _, c in xml.elements(p.device, "Context") do
				for _, a in xml.elements(c, "Action") do
					local flag = tonumber(a.attrs.Flag or "") or 0
					for _, name in { "Primary", "Secondary" } do
						local slot = xml.find(a, name)
						local what = slot and slot_output(slot)
						if what then
							local b = {
								action = id_of(p, c.attrs.ContextName or "", a.attrs.ActionName or ""),
								output = device .. ":" .. what,
								invert = is_axis(what) and bit32.band(flag, 128) ~= 0,
							}
							local k = b.action .. "|" .. b.output
							if not seen[k] then
								seen[k] = true
								table.insert(binds, b)
							end
						end
					end
				end
			end
		end
	end
	return { devices = links, binds = binds }
end)

-- ---- write -------------------------------------------------------------------------------------

-- The whitespace in front of `child` in `parent`, or nil.
local function indent_before(parent, child)
	for i, c in parent.children do
		if c == child then
			local w = parent.children[i - 1]
			if type(w) == "string" and string.match(w, "^%s*$") then
				return w
			end
			return nil
		end
	end
	return nil
end

local function set_attr(el, name, value)
	if el.attrs[name] ~= value then
		el.attrs[name] = value
	end
end

-- Gives action element `a` the outputs `list` ({what, invert}); returns the bindings before and
-- after as the sim names them, or nil when nothing changed.
local function apply(p, context, a, list)
	local before, foreign, used = {}, {}, {}
	local flag = tonumber(a.attrs.Flag or "") or 2
	for _, name in { "Primary", "Secondary" } do
		local slot = xml.find(a, name)
		if slot then
			local what = slot_output(slot)
			if what then
				table.insert(before, { slot = slot, what = what })
			else
				table.insert(foreign, slot)
				used[name] = true
			end
		end
	end
	local was, now = {}, {}
	for _, b in before do
		table.insert(was, b.what)
	end
	local inverted = false
	for _, w in list do
		table.insert(now, w.what)
		inverted = inverted or (is_axis(w.what) and w.invert == true)
	end
	table.sort(was)
	table.sort(now)
	local same = table.concat(was, ",") == table.concat(now, ",")
	local any_axis = false
	for _, w in now do
		any_axis = any_axis or is_axis(w)
	end
	if same and (not any_axis or (bit32.band(flag, 128) ~= 0) == inverted) then
		return nil
	end

	for _, b in before do
		xml.remove(a, b.slot)
	end
	-- An action without children yet indents its new ones one step deeper than itself.
	if #xml.elements(a) == 0 then
		local own = indent_before(context, a)
		if own then
			a._indent = own
			a._step = "\t"
		end
		a.children = {}
	end
	local free = {}
	for _, name in { "Primary", "Secondary" } do
		if not used[name] then
			table.insert(free, name)
		end
	end
	local written = {}
	for i, w in list do
		local name = free[i]
		if name == nil then
			plugin.log(id_of(p, context.attrs.ContextName, a.attrs.ActionName) .. ": the sim takes two bindings per action; the others are not written")
			break
		end
		local code, info = encode(w.what)
		local slot = xml.append(a, xml.element(name))
		local key = xml.append(slot, xml.element("KEY", { Information = info }))
		key.children = { tostring(code) }
		table.insert(written, w.what)
	end

	-- What kind of input the action takes now.
	if #written == 0 and #foreign == 0 then
		flag = 2
	else
		local axis = false
		for _, w in written do
			axis = axis or is_axis(w)
		end
		if axis then
			flag = bit32.bor(bit32.band(flag, bit32.bnot(3 + 128)), 4)
			if inverted then
				flag = bit32.bor(flag, 128)
			end
		elseif #written > 0 then
			if bit32.band(flag, 4) ~= 0 then
				flag = bit32.band(flag, bit32.bnot(4 + 128))
			end
			if bit32.band(flag, 3) == 0 then
				flag = bit32.bor(flag, 2)
			end
		end
	end
	set_attr(a, "Flag", tostring(flag))
	if p.full and a.attrs.Delay ~= nil then
		local bound = #written > 0 or #foreign > 0
		if not bound then
			set_attr(a, "Delay", "0.000000")
		elseif #was == 0 and #foreign == 0 and a.attrs.Delay == "0.000000" then
			-- Newly bound: the sim writes bound actions with this delay.
			set_attr(a, "Delay", "2.000000")
		end
	end

	local from, to = {}, {}
	for _, w in was do
		table.insert(from, info_of(w))
	end
	for _, w in written do
		table.insert(to, info_of(w))
	end
	return from, to
end

-- The profile with the outputs of `wanted` (id -> {{what, invert}}) on its device; returns the
-- new text and the changes.
local function rewrite(p, wanted)
	local changes = {}
	local function change(id, from, to)
		local f, t = table.concat(from, ", "), table.concat(to, ", ")
		if f == "" then
			table.insert(changes, { kind = "added", action = id, to = t })
		elseif t == "" then
			table.insert(changes, { kind = "removed", action = id, from = f })
		else
			table.insert(changes, { kind = "changed", action = id, from = f, to = t })
		end
	end
	local contexts, done = {}, {}
	for _, c in xml.elements(p.device, "Context") do
		local cname = c.attrs.ContextName or ""
		contexts[cname] = contexts[cname] or c
		for _, a in xml.elements(c, "Action") do
			local id = id_of(p, cname, a.attrs.ActionName or "")
			done[id] = true
			local from, to = apply(p, c, a, wanted[id] or {})
			if from then
				change(id, from, to)
				-- The 2020 sim keeps only bound actions.
				if not p.full and #xml.elements(a) == 0 then
					xml.remove(c, a)
				end
			end
		end
	end
	for _, id in sorted_keys(wanted) do
		if not done[id] and #wanted[id] > 0 then
			local _, cname, action = split_id(id)
			local c = contexts[cname]
			if c == nil then
				c = xml.append(p.device, xml.element("Context", { ContextName = cname }, { "ContextName" }))
				contexts[cname] = c
			end
			local a
			if p.full then
				a = xml.element(
					"Action",
					{ ActionName = action, ValueEvent = "0.000000", Delay = "2.000000", Flag = "2" },
					{ "ActionName", "ValueEvent", "Delay", "Flag" }
				)
			else
				a = xml.element("Action", { ActionName = action, Flag = "2" }, { "ActionName", "Flag" })
			end
			xml.append(c, a)
			local from, to = apply(p, c, a, wanted[id])
			if from then
				change(id, from, to)
			end
		end
	end
	return xml.write(p.doc), changes
end

local function file_name(s)
	s = string.gsub(s, '[\\/:%*%?"<>|%c]', "_")
	s = string.match(s, "^%s*(.-)[%s%.]*$")
	return if s == "" then "profile" else s
end

plugin.write(function(ctx)
	local kind, version = string.match(ctx.target.id, "^(%a+)(%d+):")
	if kind == "store" and (ctx.target.dirs or {}).import == nil then
		error("The Microsoft Store version of Flight Simulator 2020 keeps its controls in cloud saves that cannot be changed from outside the sim. Bind the controls in the sim; the table shows what the profile sends.")
	end
	local profiles = profiles_of(ctx)
	local guid_of = links_of(ctx, profiles)
	local preset = lower(string.match(tostring(ctx.options.preset or ""), "^%s*(.-)%s*$"))

	-- What each device and category gets: "guid|category" -> action id -> {{what, invert}}.
	local wanted, unlinked, skipped = {}, false, false
	for _, b in ctx.binds do
		local device, what = string.match(b.output, "^(vjoy:%d+):(.+)$")
		if device == nil then
			-- Keys and mouse buttons live in the sim's keyboard and mouse profiles.
			skipped = true
		elseif guid_of[device] == nil then
			unlinked = true
		elseif encode(what) == nil then
			plugin.log(b.action .. ": the sim cannot bind " .. b.output)
		else
			local category = split_id(b.action)
			local k = guid_of[device] .. "|" .. category
			wanted[k] = wanted[k] or {}
			wanted[k][b.action] = wanted[k][b.action] or {}
			table.insert(wanted[k][b.action], { what = what, invert = b.invert })
		end
	end
	if skipped then
		plugin.log("keyboard and mouse bindings stay as they are in the sim")
	end
	if unlinked then
		error("The sim has no control profile of the virtual joysticks yet. Open Controls in the sim, choose each virtual joystick and save its profile once, or type the device's GUID above the table.")
	end

	local files, create, changes, covered, seen = {}, {}, {}, {}, {}
	local names = {}
	for _, p in profiles do
		local guid = lower(p.guid)
		local ours = false
		for _, g in guid_of do
			ours = ours or g == guid
		end
		if ours and (preset == "" or lower(p.preset) == preset) then
			local k = guid .. "|" .. p.category
			covered[k] = true
			local text, list = rewrite(p, wanted[k] or {})
			for _, c in list do
				local s = c.kind .. c.action .. (c.from or "") .. (c.to or "")
				if not seen[s] then
					seen[s] = true
					table.insert(changes, c)
				end
			end
			if kind == "store" then
				if #list > 0 then
					local base = file_name(if p.preset ~= "" then p.preset else p.name)
						.. " - "
						.. (CATEGORY_NAMES[p.category] or p.category)
					local name, n = base .. ".xml", 1
					while names[name] do
						n += 1
						name = base .. " " .. n .. ".xml"
					end
					names[name] = true
					table.insert(create, { dir = "import", name = name, text = text })
				end
			else
				files[p.key] = text
			end
		end
	end

	local missing = {}
	for _, k in sorted_keys(wanted) do
		if not covered[k] then
			local guid, category = string.match(k, "^(.-)|(.*)$")
			table.insert(missing, guid .. " (" .. (CATEGORY_NAMES[category] or category) .. ")")
		end
	end
	if #missing > 0 then
		local which = if preset ~= "" then " named \"" .. ctx.options.preset .. "\"" else ""
		error("The sim has no control profile" .. which .. " for " .. table.concat(missing, ", ") .. ". Open Controls in the sim, choose the device (and the aircraft kind) and save a profile once.")
	end
	plugin.log(version .. ": " .. #changes .. " change(s)")
	return { files = files, create = create, changes = changes }
end)
