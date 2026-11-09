-- The API Joystick Gobloon gives game export plugins. Runs once in each plugin's state before the
-- sandbox is closed; `H` holds the host's functions and receives the hooks the plugin registers.
local H = ...

local hooks = {}
H.hooks = hooks

local function hook(name)
	return function(f)
		if type(f) ~= "function" then
			error("plugin." .. name .. " needs a function", 2)
		end
		hooks[name] = f
	end
end

plugin = {
	-- plugin.targets(function(ctx) -> {target, ...}): the game's installs or channels found.
	-- ctx.env = {platform = "windows" | "linux", roots = {folder, ...}}.
	targets = hook("targets"),
	-- plugin.version(function(ctx) -> "1.2.3" | nil): the game version of ctx.target, from
	-- ctx.files (key -> {path, text}; text is nil when the file is missing).
	version = hook("version"),
	-- plugin.read(function(ctx) -> {devices = {...}, binds = {...}}): the bindings already in the
	-- game's files. ctx: target, files, catalog, ours (our virtual devices).
	read = hook("read"),
	-- plugin.write(function(ctx) -> {files = {key = text}, changes = {...}}): the game's files with
	-- the profile's bindings. ctx: target, files, catalog, ours, devices, binds, profile.
	write = hook("write"),
	log = function(text)
		H.log(tostring(text))
	end,
}
