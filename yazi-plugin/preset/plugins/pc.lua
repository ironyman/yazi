-- This PC: drives and cloud storage folders, listed under `pc://this/` and rerouted to their real locations

local M = {}

-- T_DIR | 0o755
local DIR = 0x41ED

-- Links to its real location, so This PC can hover the place in the parent column when inside it
local function dir(url, place)
	local stat = Stat { mode = DIR, len = place and place.len }
	return File { url = url, stat = stat, lstat = stat, link_to = place and Url(place.path).path }
end

local function is_dir(path)
	local file = fs.file(Url(path))
	return file and file.stat.is_dir
end

--- Drives and cloud storage folders, each with a unique `name` and its real `path`.
local function places()
	local list, seen = {}, {}
	local function add(name, path, len)
		if name and path and not seen[name] and not seen[path] then
			seen[name], seen[path] = true, true
			list[#list + 1] = { name = name, path = path, len = len or 0 }
		end
	end

	-- Local drives first, then network ones, then cloud storage
	local partitions = fs.partitions()
	for _, remote in ipairs { false, true } do
		for _, p in ipairs(partitions) do
			if p.dist and (p.external == true) == remote then
				local root = tostring(p.dist)
				local label = p.label or (remote and "Network Drive") or (p.removable and "Removable Disk") or "Local Disk"
				add(string.format("%s (%s)", label, root:match("^%a:") or root), root, p.capacity)
			end
		end
	end

	local function cloud(name, path)
		if path and is_dir(path) then
			add(name, path)
		end
	end

	for _, var in ipairs { "OneDriveConsumer", "OneDriveCommercial", "OneDrive" } do
		local path = os.getenv(var)
		cloud(path and Url(path).name, path)
	end

	for _, var in ipairs { "APPDATA", "LOCALAPPDATA" } do
		local base = os.getenv(var)
		local f = base and io.open(tostring(Url(base):join("Dropbox/info.json")))
		if f then
			local accounts = ya.json_decode(f:read("a"))
			f:close()
			for kind, account in pairs(type(accounts) == "table" and accounts or {}) do
				local path = type(account) == "table" and account.path
				cloud(kind == "personal" and "Dropbox" or (path and Url(path).name), path)
			end
		end
	end

	local home = os.getenv("USERPROFILE") or os.getenv("HOME")
	for _, c in ipairs {
		{ "Google Drive", "Google Drive" },
		{ "Google Drive", "My Drive" },
		{ "Dropbox", "Dropbox" },
		{ "iCloud Drive", "iCloudDrive" },
		{ "Box", "Box" },
	} do
		cloud(c[1], home and tostring(Url(home):join(c[2])))
	end

	return list
end

--- The place `url` lies in, the real URL it stands for, and whether it's the place itself.
local function resolve(url)
	local name, rest = tostring(url.path):match("^/?([^/]+)/?(.*)$")
	for _, p in ipairs(name and places() or {}) do
		if p.name == name then
			local real = Url(p.path)
			return p, rest == "" and real or real:join(rest), rest == ""
		end
	end
end

local function file(url)
	local place, real, top = resolve(url)
	if not place then
		return dir(url)
	elseif top then
		return dir(url, place)
	end

	local f, err = fs.file(real)
	return f and File { url = url, stat = f.stat, lstat = f.lstat, link_to = f.link_to }, err
end

function M:provide(job)
	local op = job.op
	if op == "Capabilities" then
		return { reroute = 3 }
	elseif op == "Absolute" or op == "Canonicalize" or op == "Casefold" then
		return job.url
	elseif op == "File" then
		return file(job.url)
	elseif op == "Revalidate" then
		return file(job.file.url)
	elseif op == "Reroute" then
		local _, real = resolve(job.url)
		if real then
			return fs.file(real)
		end
		return nil, Error.fs { kind = "Unsupported", message = "This PC has no real location" }
	elseif op ~= "ReadDir" then
		return false, Err("pc:// does not support %s", op)
	end

	return ya.co(function()
		local _, real = resolve(job.url)
		if not real then
			for _, p in ipairs(places()) do
				coroutine.yield(dir(job.url:join(p.name), p))
			end
			return
		end

		-- Lists the place itself, so hovering it in This PC previews its contents
		local files, err = fs.read_dir(real, { resolve = true })
		if not files then
			return nil, err
		end
		for _, f in ipairs(files) do
			coroutine.yield(File { url = job.url:join(f.name), stat = f.stat, lstat = f.lstat, link_to = f.link_to })
		end
	end)
end

return M
