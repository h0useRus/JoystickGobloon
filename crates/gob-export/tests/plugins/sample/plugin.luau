-- A made-up game: <root>/Sample/version.txt holds the version, <root>/Sample/binds.txt
-- "action = output" lines; other lines are kept as they are.

plugin.targets(function(ctx)
	local found = {}
	for _, root in ctx.env.roots do
		table.insert(found, {
			id = root,
			name = "Sample in " .. root,
			files = { binds = root .. "/Sample/binds.txt", version = root .. "/Sample/version.txt" },
			version_file = "version",
		})
	end
	return found
end)

plugin.version(function(ctx)
	local f = ctx.files[ctx.target.version_file]
	if f == nil or f.text == nil then
		return nil
	end
	return (string.gsub(f.text, "%s+", ""))
end)

local function parse(text)
	local binds = {}
	for line in string.gmatch(text or "", "[^\n]+") do
		local action, output = string.match(line, "^(%S+)%s*=%s*(%S+)$")
		if action then
			binds[action] = output
		end
	end
	return binds
end

plugin.read(function(ctx)
	local binds = {}
	for action, output in parse(ctx.files.binds.text) do
		table.insert(binds, { action = action, output = output })
	end
	table.sort(binds, function(a, b)
		return a.action < b.action
	end)
	return { binds = binds }
end)

plugin.write(function(ctx)
	local old = parse(ctx.files.binds.text)
	local new, changes, lines = {}, {}, {}
	for _, b in ctx.binds do
		new[b.action] = b.output
	end
	-- Keep foreign lines and the order of known actions; change or drop bound ones.
	for line in string.gmatch(ctx.files.binds.text or "", "[^\n]+") do
		local action = string.match(line, "^(%S+)%s*=")
		if action == nil then
			table.insert(lines, line)
		elseif new[action] ~= nil then
			if new[action] ~= old[action] then
				table.insert(changes, { kind = "changed", action = action, from = old[action], to = new[action] })
			end
			table.insert(lines, action .. " = " .. new[action])
			new[action] = nil
		else
			table.insert(changes, { kind = "removed", action = action, from = old[action] })
		end
	end
	local added = {}
	for action in new do
		table.insert(added, action)
	end
	table.sort(added)
	for _, action in added do
		table.insert(changes, { kind = "added", action = action, to = new[action] })
		table.insert(lines, action .. " = " .. new[action])
	end
	return { files = { binds = table.concat(lines, "\n") .. "\n" }, changes = changes }
end)
