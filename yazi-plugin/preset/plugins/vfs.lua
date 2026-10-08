local M = {}

local EXTRACT_LIMIT = 50 * 1024 * 1024

function M:peek(job)
	local text = "Remote file, download to preview"
	if job.file.url.spec.scheme == "archive" and job.file.stat.len <= EXTRACT_LIMIT then
		text = "Extracting to preview..."
		ya.emit("download", { job.file.url })
	end

	local line = ui.Line(text):reverse()
	ya.preview_widget(job, ui.Text(line):area(job.area):wrap(ui.Wrap.YES))
end

function M:seek() end

function M:spot(job) require("file"):spot(job) end

return M
