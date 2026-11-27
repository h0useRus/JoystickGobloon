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
	-- ctx.env = {platform = "windows" | "linux", roots = {folder, ...}, documents = "C:/.../",
	-- local_app_data = "C:/.../"} (the user's Documents and local application data folders, ""
	-- when not known).
	targets = hook("targets"),
	-- plugin.version(function(ctx) -> "1.2.3" | nil): the game version of ctx.target, from
	-- ctx.files (key -> {path, text}; text is nil when the file is missing).
	version = hook("version"),
	-- plugin.read(function(ctx) -> {devices = {...}, binds = {...}}): the bindings already in the
	-- game's files. ctx: target, files, catalog, ours (our virtual devices: output, name, guid,
	-- instance, axes = {"x", "rz", ...} in DirectInput order, buttons, hats), devices (the links the profile keeps: {output, game_device}).
	read = hook("read"),
	-- plugin.write(function(ctx) -> {files = {key = text}, changes = {...}}): the game's files with
	-- the profile's bindings. ctx: target, files, catalog, ours, devices, binds, profile.
	write = hook("write"),
	log = function(text)
		H.log(tostring(text))
	end,
	-- plugin.options { key = {kind = "bool" | "number" | "text", label = "...", default = ...} }:
	-- a form the program shows; write hooks get the values in ctx.options.
	options = function(decls)
		if type(decls) ~= "table" then
			error("plugin.options needs a table", 2)
		end
		H.options = decls
	end,
}

-- XML that keeps the file's formatting: elements are {tag, attrs, children}, text is a string,
-- comments and declarations are {raw}. Elements left alone are written back byte for byte.
xml = {
	parse = function(text)
		return H.xml_parse(text)
	end,
	write = function(doc)
		return H.xml_write(doc)
	end,
}

-- The child elements of an element (or a document), in order.
function xml.elements(el, tag)
	local out = {}
	for _, c in el.children or {} do
		if type(c) == "table" and c.tag ~= nil and (tag == nil or c.tag == tag) then
			table.insert(out, c)
		end
	end
	return out
end

-- The first child element with this tag, and with every attribute of `attrs` when given.
function xml.find(el, tag, attrs)
	for _, c in xml.elements(el, tag) do
		local ok = true
		for k, v in attrs or {} do
			if c.attrs[k] ~= v then
				ok = false
				break
			end
		end
		if ok then
			return c
		end
	end
	return nil
end

function xml.root(doc)
	return xml.elements(doc)[1]
end

-- A new element; `order` lists attribute names in the order they are written.
function xml.element(tag, attrs, order)
	return { tag = tag, attrs = attrs or {}, order = order, children = {} }
end

-- Adds `child` as the last element of `parent`, indented like its other children. New elements
-- remember their indentation, so children added to them later are indented one step deeper.
function xml.append(parent, child)
	local list = parent.children
	local n = #list
	local tail = if type(list[n]) == "string" and string.match(list[n], "^%s*$") then list[n] else nil
	local had_tail = tail ~= nil
	local indent = nil
	for i = n, 1, -1 do
		if type(list[i]) == "table" and list[i].tag ~= nil then
			if type(list[i - 1]) == "string" and string.match(list[i - 1], "^%s*$") then
				indent = list[i - 1]
			end
			break
		end
	end
	local step = parent._step
	if indent == nil then
		if tail ~= nil then
			indent = tail .. (step or "	")
		elseif parent._indent ~= nil then
			indent = parent._indent .. (step or "	")
			tail = parent._indent
		end
	end
	if indent ~= nil and tail ~= nil and #indent > #tail and string.sub(indent, 1, #tail) == tail then
		step = string.sub(indent, #tail + 1)
	end
	if had_tail then
		table.remove(list)
	end
	if indent ~= nil then
		table.insert(list, indent)
	end
	table.insert(list, child)
	if tail ~= nil then
		table.insert(list, tail)
	end
	if type(child) == "table" then
		child._indent = indent
		child._step = step
	end
	return child
end

-- Removes `child` and the whitespace in front of it.
function xml.remove(parent, child)
	local list = parent.children
	for i, c in list do
		if c == child then
			-- Remember the indentation step, for elements appended later.
			local indent, tail = list[i - 1], list[#list]
			if
				type(indent) == "string"
				and type(tail) == "string"
				and #indent > #tail
				and string.sub(indent, 1, #tail) == tail
			then
				parent._step = string.sub(indent, #tail + 1)
			end
			table.remove(list, i)
			if type(list[i - 1]) == "string" and string.match(list[i - 1], "^%s*$") then
				table.remove(list, i - 1)
			end
			return true
		end
	end
	return false
end

json = {
	parse = function(text)
		return H.json_parse(text)
	end,
	write = function(value)
		return H.json_write(value)
	end,
}

-- INI text -> {section = {key = value}}; keys before any section are under "".
ini = {
	parse = function(text)
		return H.ini_parse(text)
	end,
}

-- Lua data files: the first table constructor in the text ("local diff = {...} return diff"), read
-- without running anything; write gives a table constructor with sorted keys and tab indentation.
luadata = {
	parse = function(text)
		return H.luadata_parse(text)
	end,
	write = function(value)
		return H.luadata_write(value)
	end,
}

-- version.compare("4.10.197", "4.9") -> 1 (also 0 and -1); missing segments count as 0.
version = {
	compare = function(a, b)
		return H.version_compare(a, b)
	end,
}
