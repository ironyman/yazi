Tabs = {
	_id = "tabs",
	_offsets = {},
}

function Tabs:new(area)
	return setmetatable({
		_area = area,
	}, { __index = self })
end

function Tabs:reflow() return { self } end

function Tabs:redraw()
	if self.height() < 1 then
		return {}
	end

	local style = self:style()
	local lines = {
		self:sep(th.tabs.sep_outer.open, style.inactive, ui.Style():fg(style.inactive:bg())),
	}

	local pos = lines[1]:width()
	local max = math.floor(self:inner_width() / #cx.tabs)
	for i = 1, #cx.tabs do
		local name = ui.truncate(string.format(" %d %s ", i, cx.tabs[i].name), { max = max })
		if i == cx.tabs.idx then
			local cap = ui.Style():fg(style.active:bg()):bg(style.inactive:bg())
			lines[#lines + 1] = ui.Line {
				self:sep(th.tabs.sep_inner.open, style.active, cap),
				ui.Span(name):style(style.active),
				self:sep(th.tabs.sep_inner.close, style.active, cap),
			}
		else
			lines[#lines + 1] = ui.Line(name):style(style.inactive)
		end
		self._offsets[i], pos = pos, pos + lines[#lines]:width()
	end

	lines[#lines + 1] = self:sep(th.tabs.sep_outer.close, style.inactive, ui.Style():fg(style.inactive:bg()))
	return ui.Line(lines):area(self._area)
end

-- A blank separator is filled with `fill`, so the shape stays rectangular;
-- otherwise it's a glyph drawn in `cap` to round the edge.
function Tabs:sep(s, fill, cap) return ui.Line(s):style(s:match("^%s*$") and fill or cap) end

function Tabs.height() return #cx.tabs > 1 and 1 or 0 end

function Tabs:style()
	local s = ui.Style():fg("reset"):bg("reset")
	return {
		active = s:patch(th.tabs.active),
		inactive = s:patch(th.tabs.inactive),
	}
end

function Tabs:inner_width()
	local si, so = th.tabs.sep_inner, th.tabs.sep_outer
	return math.max(0, self._area.w - ui.Line({ si.open, si.close, so.open, so.close }):width())
end

-- Mouse events
function Tabs:click(event, up)
	if up then
		return
	elseif not event.is_left and not event.is_right then
		return
	end

	for i = #cx.tabs, 1, -1 do
		if event.x >= self._offsets[i] then
			ya.emit("tab_switch", { i - 1 })
			break
		end
	end
end

function Tabs:scroll(event, step) end

function Tabs:touch(event, step) end
