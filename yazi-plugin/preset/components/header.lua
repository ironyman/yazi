Header = {
	-- TODO: remove these two constants
	LEFT = 0,
	RIGHT = 1,

	_id = "header",
	_inc = 1000,
	_left = {
		{ "cwd", id = 1, order = 1000 },
	},
	_right = {
		{ "count", id = 1, order = 1000 },
		{ "meters", id = 2, order = 2000 },
	},
	-- Where the count was drawn and which files it counts, for clicks on it
	_counted = {},
}

function Header:new(area, tab)
	return setmetatable({
		_area = area,
		_tab = tab,
		_current = tab.current,
	}, { __index = self })
end

function Header:cwd()
	local max = self._area.w - self._right_width
	if max <= 0 then
		return ""
	end

	local s = ya.readable_path(tostring(self._current.cwd)) .. self:flags()
	return ui.Span(ui.truncate(s, { max = max, rtl = true })):style(th.mgr.cwd)
end

function Header:flags()
	local cwd = self._current.cwd
	local data = cwd.spec.is_view and cwd.spec.data
	local filter = self._current.files.filter
	local finder = self._tab.finder

	local t = {}
	if type(data) == "table" then
		t[#t + 1] = string.format("search: %s", data[1])
	end
	if filter then
		t[#t + 1] = string.format("filter: %s", filter)
	end
	if finder then
		t[#t + 1] = string.format("find: %s", finder)
	end

	local staged = fs.archive.summary(cwd)
	if staged and staged.added + staged.modified + staged.removed > 0 then
		local state = staged.committing and "committing" or "uncommitted"
		t[#t + 1] = string.format("%s: +%d ~%d -%d", state, staged.added, staged.modified, staged.removed)
	elseif staged and staged.committing then
		t[#t + 1] = "committing"
	end
	return #t == 0 and "" or " (" .. table.concat(t, ", ") .. ")"
end

function Header:count()
	local selected = #self._tab.selected
	local yanked = selected > 0 and 0 or #cx.yanked
	self._counted.width = ui.width(string.format(" %d ", math.max(selected, yanked)))

	local span
	if selected > 0 then
		span = ui.Span(" " .. selected .. " "):style(th.mgr.count_selected)
		self._counted.mode = "marked"
	elseif yanked <= 0 then
		self._counted.mode = nil
		return ""
	elseif cx.yanked.is_cut then
		span = ui.Span(" " .. yanked .. " "):style(th.mgr.count_cut)
		self._counted.mode = "yanked"
	else
		span = ui.Span(" " .. yanked .. " "):style(th.mgr.count_copied)
		self._counted.mode = "yanked"
	end

	return ui.Line { span, " " }
end

function Header:meters()
	local spans = {}
	for _, part in ipairs(cx.meters) do
		local span = ui.Span(part.text)
		if not part.percent then
			span = span:dim()
		elseif part.percent >= 90 then
			span = span:fg("red"):bold()
		elseif part.percent >= 70 then
			span = span:fg("yellow"):bold()
		else
			span = span:fg("green"):bold()
		end
		spans[#spans + 1] = span
	end

	if #spans == 0 then
		return ""
	end
	spans[#spans + 1] = " "
	return ui.Line(spans)
end

function Header:reflow() return { self } end

function Header:redraw()
	local right = self:children_redraw(self.RIGHT)
	self._right_width = right:width()
	self._counted.x = self._area.x + math.max(0, self._area.w - self._right_width)

	local left = self:children_redraw(self.LEFT)

	return {
		ui.Line(left):area(self._area),
		ui.Line(right):area(self._area):align(ui.Align.RIGHT),
	}
end

-- Mouse events
function Header:click(event, up)
	local counted = self._counted
	if up or not event.is_left or not counted.mode then
		return
	elseif event.x >= counted.x and event.x < counted.x + counted.width then
		ya.emit("palette", { counted.mode })
	end
end

function Header:scroll(event, step) end

function Header:touch(event, step) end

-- Children
function Header:children_add(fn, order, side)
	self._inc = self._inc + 1
	local children = side == self.RIGHT and self._right or self._left

	children[#children + 1] = { fn, id = self._inc, order = order }
	table.sort(children, function(a, b) return a.order < b.order end)

	return self._inc
end

function Header:children_remove(id, side)
	local children = side == self.RIGHT and self._right or self._left
	for i, child in ipairs(children) do
		if child.id == id then
			table.remove(children, i)
			break
		end
	end
end

function Header:children_redraw(side)
	local lines = {}
	for _, c in ipairs(side == self.RIGHT and self._right or self._left) do
		lines[#lines + 1] = (type(c[1]) == "string" and self[c[1]] or c[1])(self)
	end
	return ui.Line(lines)
end
