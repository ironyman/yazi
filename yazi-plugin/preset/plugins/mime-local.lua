-- stylua: ignore
local TYPE_PATS = { "text", "image", "video", "application", "audio", "font", "inode", "message", "model", "vector", "biosig", "chemical", "rinex", "x%-epoc" }

-- Fallbacks for when `file` is not installed
-- stylua: ignore
local EXTS = {
	json = "application/json", js = "application/javascript", mjs = "application/javascript", cjs = "application/javascript",
	svg = "image/svg+xml", bmp = "image/bmp", ico = "image/microsoft.icon", mp3 = "audio/mpeg",
	docx = "application/openxmlformats-officedocument.wordprocessingml.document",
	xlsx = "application/openxmlformats-officedocument.spreadsheetml.sheet",
	pptx = "application/openxmlformats-officedocument.presentationml.presentation",
	odt = "application/oasis.opendocument.text", ods = "application/oasis.opendocument.spreadsheet",
	epub = "application/epub+zip", apk = "application/android.package-archive",
	deb = "application/debian.binary-package", rpm = "application/rpm", iso = "application/iso9660-image",
}

-- stylua: ignore
local MAGICS = {
	{ 0, "\x89PNG\r\n\x1a\n", "image/png" }, { 0, "\xff\xd8\xff", "image/jpeg" },
	{ 0, "GIF87a", "image/gif" }, { 0, "GIF89a", "image/gif" }, { 8, "WEBP", "image/webp" },
	{ 0, "II*\0", "image/tiff" }, { 0, "MM\0*", "image/tiff" },
	{ 4, "ftypavif", "image/avif" }, { 4, "ftypheic", "image/heic" }, { 4, "ftypmif1", "image/heif" },
	{ 4, "ftypqt  ", "video/quicktime" }, { 4, "ftypM4A ", "audio/m4a" }, { 4, "ftyp", "video/mp4" },
	{ 0, "\x1a\x45\xdf\xa3", "video/matroska" }, { 8, "AVI ", "video/msvideo" },
	{ 0, "ID3", "audio/mpeg" }, { 0, "fLaC", "audio/flac" }, { 0, "OggS", "audio/ogg" }, { 8, "WAVE", "audio/wav" },
	{ 0, "%PDF-", "application/pdf" }, { 0, "PK\x03\x04", "application/zip" }, { 0, "\x1f\x8b", "application/gzip" },
	{ 0, "7z\xbc\xaf\x27\x1c", "application/7z-compressed" }, { 0, "Rar!\x1a\x07", "application/rar" },
	{ 0, "\xfd7zXZ\0", "application/xz" }, { 0, "\x28\xb5\x2f\xfd", "application/zstd" },
	{ 0, "BZh", "application/bzip2" }, { 257, "ustar", "application/tar" },
	{ 0, "SQLite format 3\0", "application/sqlite3" },
	{ 0, "wOFF", "font/woff" }, { 0, "wOF2", "font/woff2" }, { 0, "OTTO", "font/otf" }, { 0, "\0\1\0\0\0", "font/sfnt" },
}

local M = {}

function M:fetch(job)
	return ya.co(function()
		local paths, updates = {}, {}
		for i, file in ipairs(job.files) do
			paths[i] = tostring(file.path)
		end

		local flush = ya.throttle(0.3, function()
			if next(updates) then
				ya.emit("update_mimes", { updates = updates })
				updates = {}
			end
		end)

		local child, err = M.spawn_file1(paths)
		if not child then
			return M.fallback(err, job.files)
		end

		local i, f, match, ignore = 1, nil, nil, nil
		repeat
			local line, event = child:read_line_with { timeout = 300 }
			if event == 3 then
				flush(true)
				goto continue
			elseif event ~= 0 then
				break
			end

			f, match, ignore = job.files[i], M.match_mimetype(line)
			if match then
				if coroutine.yield(f, { match }) and not f.stat.is_dummy then
					updates[f.url] = match
					flush()
				end
				i = i + 1
			elseif not ignore then
				coroutine.yield(f, {
					error = Err("Failed to determine MIME type for `%s`", f.url),
					retry = true,
				})
				i = i + 1
			end
			::continue::
		until i > #paths

		for j = i, #paths do
			coroutine.yield(job.files[j], {
				error = Err("Failed to read `file` output"),
				retry = true,
			})
		end
		flush(true)
	end)
end

function M.match_mimetype(line)
	for _, pat in ipairs(TYPE_PATS) do
		local typ, sub = line:match(string.format("(%s/)([+-.a-zA-Z0-9]+)%%s+$", pat))
		if not sub then
		elseif line:find(typ .. sub, 1, true) == 1 then
			return typ:gsub("^x%-", "", 1) .. sub:gsub("^x%-", "", 1):gsub("^vnd%.", "", 1)
		else
			return nil, true
		end
	end
end

function M.file1_bin() return os.getenv("YAZI_FILE_ONE") or "file" end

function M.spawn_file1(paths)
	local bin = M.file1_bin()
	local windows = ya.target_family() == "windows"

	local cmd = Command(bin):arg({ "-bL", "--mime-type" }):stdout(Command.PIPED)
	if windows then
		cmd:arg({ "-f", "-" }):stdin(Command.PIPED)
	else
		cmd:arg("--"):arg(paths)
	end

	local child, err = cmd:spawn()
	if not child then
		local e = Error.fs {
			kind = err.kind or "Other",
			code = err.code,
			message = string.format("Failed to start `%s`, error: %s", bin, err),
		}
		return nil, e
	elseif windows then
		child:write_all(table.concat(paths, "\n"))
		child:flush()
		ya.drop(child:take_stdin())
	end

	return child
end

function M.fallback(err, files)
	local updates = {}
	for _, file in ipairs(files) do
		local mime = err.kind == "NotFound" and M.guess(file)
		if not mime then
			coroutine.yield(file, { error = Error(err), retry = true })
		elseif coroutine.yield(file, { mime }) and not file.stat.is_dummy then
			updates[file.url] = mime
		end
	end
	return require("mime.dir").commit(updates)
end

function M.guess(file)
	local mime = EXTS[(file.url.ext or ""):lower()]
	if file.stat.len == 0 then
		return "inode/empty"
	elseif mime then
		return mime
	end

	local head = M.read_head(file.url)
	return (head and M.sniff(head))
		or (head and not head:find("\0", 1, true) and "text/plain")
		or "error/file1-not-found"
end

function M.sniff(head)
	for _, m in ipairs(MAGICS) do
		local off, magic, mime = m[1], m[2], m[3]
		if head:sub(off + 1, off + #magic) == magic then
			return mime
		end
	end
end

function M.read_head(url)
	local fd = fs.access():read(true):open(url)
	if fd then
		local head = fd:read(512)
		ya.drop(fd)
		return head
	end
end

return M
