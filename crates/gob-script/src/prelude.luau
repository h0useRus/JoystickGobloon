-- The API Joystick Gobloon gives Lua scripts. Runs once in each script's state before the
-- sandbox is closed; `H` holds the host's functions (`call` is set only while a handler runs).
local H = ...

local function call(op, ...)
	local host = H.call
	if host == nil then
		error("'" .. op .. "' works only inside handlers and periodic functions", 3)
	end
	return host(op, ...)
end

-- A mode argument: nil (every mode), a name, or a mode variable.
local function mode_name(m)
	if type(m) == "table" then
		return m.value
	end
	return m
end

-- A key argument: a key variable, {scan = , extended = } or a scan code.
local function key_of(k)
	if type(k) == "table" then
		if k.value ~= nil and type(k.value) == "table" then
			k = k.value
		end
		return k.scan, k.extended == true
	end
	return k, false
end

local function input(kind)
	return function(spec)
		local value = H.declare(kind, spec)
		local v = { name = spec.name, value = value }
		if value ~= nil then
			v.device, v.type, v.id = value.device, value.type, value.id
		end
		function v:on(mode, fn)
			if fn == nil then
				mode, fn = nil, mode
			end
			if self.value ~= nil then
				H.on(self.value.device, self.value.type, self.value.id, mode_name(mode), fn)
			end
		end
		return v
	end
end

local function plain(kind)
	return function(spec)
		return { name = spec.name, value = H.declare(kind, spec) }
	end
end

script = {
	bool = plain("bool"),
	int = plain("int"),
	float = plain("float"),
	string = plain("string"),
	selection = plain("selection"),
	mode = plain("mode"),
	physical_input = input("physical-input"),
	logical_input = input("logical-device"),
}

function script.key(spec)
	local v = { name = spec.name, value = H.declare("keyboard", spec) }
	function v:on(mode, fn)
		if fn == nil then
			mode, fn = nil, mode
		end
		if self.value ~= nil then
			H.on_key(self.value.scan, self.value.extended, mode_name(mode), fn)
		end
	end
	return v
end

function script.virtual_output(spec)
	local value = H.declare("vjoy", spec)
	local v = { name = spec.name, value = value }
	if value ~= nil then
		v.device, v.type, v.id = value.device, value.type, value.id
	end
	-- Axis -1..1, button true/false, hat degrees (0 = up) or nil for centered.
	function v:set(x)
		local o = self.value
		if o == nil then
			return
		end
		if o.type == "axis" then
			call("vjoy_axis", o.device, o.id, x)
		elseif o.type == "button" then
			call("vjoy_button", o.device, o.id, x == true)
		else
			call("vjoy_hat", o.device, o.id, x)
		end
	end
	return v
end

-- Runs `fn` every `seconds` (at least 0.01) while the profile runs.
function script.every(seconds, fn)
	H.every(seconds, fn)
end

-- Any input by device GUID: script.on_input(guid, "button", 3, "Default", fn).
function script.on_input(device, kind, id, mode, fn)
	if fn == nil then
		mode, fn = nil, mode
	end
	H.on(device, kind, id, mode_name(mode), fn)
end

vjoy = {
	axis = function(device, axis, value)
		call("vjoy_axis", device, axis, value)
	end,
	button = function(device, button, pressed)
		call("vjoy_button", device, button, pressed == true)
	end,
	hat = function(device, hat, degrees)
		call("vjoy_hat", device, hat, degrees)
	end,
}

gamepad = {
	button = function(pad, control, pressed)
		call("pad_button", pad, control, pressed == true)
	end,
	axis = function(pad, control, value)
		call("pad_axis", pad, control, value)
	end,
}

keyboard = {
	down = function(k)
		local scan, ext = key_of(k)
		call("key", scan, ext, true)
	end,
	up = function(k)
		local scan, ext = key_of(k)
		call("key", scan, ext, false)
	end,
	tap = function(k)
		local scan, ext = key_of(k)
		call("key", scan, ext, true)
		call("key", scan, ext, false)
	end,
}

mouse = {
	button = function(name, pressed)
		call("mouse_button", name, pressed == true)
	end,
	move = function(dx, dy)
		call("mouse_move", dx, dy)
	end,
	wheel = function(notches)
		call("mouse_wheel", notches)
	end,
}

mode = {
	current = function()
		return call("mode")
	end,
	switch = function(name)
		call("switch_mode", mode_name(name))
	end,
}

-- Current state of a physical input: joystick.axis(stick.device, 1).
joystick = {
	axis = function(device, id)
		return call("axis", device, id)
	end,
	button = function(device, id)
		return call("button", device, id)
	end,
	hat = function(device, id)
		return call("hat", device, id)
	end,
}

function speak(text)
	call("speak", tostring(text))
end

function play_sound(file, volume)
	call("sound", file, volume or 100)
end

-- Runs the action with this name (its card title) like a short press of its input.
function run_action(name)
	return call("run_action", name)
end

function log(...)
	local parts = table.pack(...)
	for i = 1, parts.n do
		parts[i] = tostring(parts[i])
	end
	H.log(table.concat(parts, " ", 1, parts.n))
end

print = log
