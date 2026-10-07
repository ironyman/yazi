local M = {}

function M:entry()
	local editor = M.editor()
	if not editor then
		return ya.notify {
			title = "Edit",
			content = "No editor found, set $EDITOR or install nvim/vim/vi",
			timeout = 5,
			level = "error",
		}
	end
	ya.emit("shell", { editor .. " %s", block = true })
end

function M.editor()
	local editor = os.getenv("EDITOR")
	if editor and editor ~= "" then
		return editor
	end

	local windows = ya.target_family() == "windows"
	for _, name in ipairs { "nvim", "vim", "vi", windows and "edit" or nil } do
		if M.exists(name, windows) then
			return name
		end
	end
end

function M.exists(name, windows)
	local exts = { "" }
	if windows then
		for ext in (os.getenv("PATHEXT") or ".COM;.EXE;.BAT;.CMD"):gmatch("[^;]+") do
			exts[#exts + 1] = ext
		end
	end

	for dir in (os.getenv("PATH") or ""):gmatch(windows and "[^;]+" or "[^:]+") do
		for _, ext in ipairs(exts) do
			local cha = fs.stat(Url(dir):join(name .. ext))
			if cha and not cha.is_dir then
				return true
			end
		end
	end
	return false
end

return M
