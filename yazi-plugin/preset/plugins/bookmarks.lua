local M = {}

local KEYS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"

local function notify(content, level)
	ya.notify { title = "Bookmarks", content = content, timeout = 3, level = level or "info" }
end

local function sorted(marks)
	local list = {}
	for _, mark in pairs(marks or {}) do
		list[#list + 1] = mark
	end
	table.sort(list, function(a, b) return KEYS:find(a.on, 1, true) < KEYS:find(b.on, 1, true) end)
	return list
end

local get = ya.sync(function(st) return st.marks or {} end)

local set = ya.sync(function(st, marks)
	st.marks = sorted(marks)
	ps.pub_to(0, "@bookmarks", st.marks)
end)

local target = ya.sync(function()
	local folder = cx.active.current
	if folder.hovered then
		return { path = tostring(folder.hovered.url), reveal = true }
	end
	return { path = tostring(folder.cwd), reveal = false }
end)

function M:setup()
	ps.sub_remote("@bookmarks", function(marks) self.marks = sorted(marks) end)
end

function M:entry(job)
	local action = job.args[1]
	if action == "save" then
		M.save()
	elseif action == "jump" then
		M.jump()
	elseif action == "delete" then
		M.delete()
	elseif action == "delete_all" then
		set {}
		notify("Deleted all bookmarks")
	end
end

function M.save()
	local cands = {}
	for i = 1, #KEYS do
		cands[i] = { on = KEYS:sub(i, i) }
	end

	notify("Press a key (0-9, A-Z, a-z) to save the bookmark, or <Esc> to cancel")
	local idx = ya.which { cands = cands, silent = true }
	if not idx then
		return
	end

	local mark = target()
	mark.on = cands[idx].on

	local marks = {}
	for _, m in ipairs(get()) do
		if m.on ~= mark.on then
			marks[#marks + 1] = m
		end
	end
	marks[#marks + 1] = mark

	set(marks)
	notify(string.format("Bookmark '%s' -> %s", mark.on, mark.path))
end

function M.jump()
	local marks = get()
	local idx = M.pick(marks)
	if idx then
		ya.emit(marks[idx].reveal and "reveal" or "cd", { marks[idx].path, raw = true })
	end
end

function M.delete()
	local marks = get()
	local idx = M.pick(marks)
	if idx then
		local mark = table.remove(marks, idx)
		set(marks)
		notify(string.format("Deleted bookmark '%s'", mark.on))
	end
end

function M.pick(marks)
	if #marks == 0 then
		return notify("No bookmarks", "warn")
	end

	local cands = {}
	for i, mark in ipairs(marks) do
		cands[i] = { on = mark.on, desc = mark.path }
	end
	return ya.which { cands = cands }
end

return M
