-- widget.luau — бар-виджет P0: глиф состояния + "Трек — Артист".
-- Без анимаций: обновление раз в секунду только при воспроизведении.

local ymd = require("./lib/ymd.luau")

local s = {
  status = "off",
  title = "",
  artists = {},
  position = 0,
  duration = 0,
  rev = -1,
}

local GLYPH = {
  playing = "player-play",
  paused = "player-pause",
  idle = "music",
  off = "music-off",
  error = "alert-triangle",
}

local function safe_set_text(text)
  local ok = pcall(barWidget.setText, text)
  if not ok then
    noctalia.log("[ym] barWidget.setText unavailable")
  end
end

local function safe_set_glyph(name)
  local ok = pcall(barWidget.setGlyph, name)
  if not ok then
    noctalia.log("[ym] barWidget.setGlyph unavailable: " .. name)
  end
end

local function render()
  safe_set_glyph(GLYPH[s.status] or GLYPH.idle)

  if s.status == "off" then
    safe_set_text("ymd off")
    return
  end

  if s.title == "" then
    safe_set_text("")
    return
  end

  local line = s.title
  if s.artists[1] then
    line = line .. " — " .. s.artists[1]
  end
  if #line > 34 then
    line = line:sub(1, 33) .. "…"
  end
  safe_set_text(line)
end

local function apply(state)
  if type(state) ~= "table" then
    return
  end
  if (state.rev or -1) <= s.rev then
    return
  end

  s.rev = state.rev or 0
  s.status = state.status or "idle"
  s.position = state.positionMs or 0

  if state.track then
    s.title = state.track.title or ""
    s.artists = state.track.artists or {}
    s.duration = state.track.durationMs or 0
  else
    s.title = ""
    s.artists = {}
    s.duration = 0
  end

  render()
end

function update()
  noctalia.setUpdateInterval(s.status == "playing" and 1000 or 3000)
  ymd.get("/v1/state", function(state, err)
    if err then
      s.status = "off"
      safe_set_glyph(GLYPH.off)
      safe_set_text("ymd off")
      return
    end
    apply(state)
  end)
end

function onClick()
  ymd.cmd("toggle")
end

function onRightClick()
  noctalia.togglePanel("pbolkhovitin/yandex-music:panel")
end

function onMiddleClick()
  ymd.cmd("like")
end

function onScroll(axis, steps, startsGesture)
  if axis ~= "vertical" or not startsGesture then
    return
  end
  if steps < 0 then
    ymd.cmd("prev")
  else
    ymd.cmd("next")
  end
end

function onIpc(event, payload)
  noctalia.log("[ym] ipc " .. tostring(event))
  update()
end

function onEnable()
  noctalia.log("[ym] widget enabled")
end
