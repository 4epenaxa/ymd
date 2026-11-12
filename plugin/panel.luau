-- panel.luau — S3: декларативное ui.* дерево в панели.

local ymd = require("./lib/ymd.luau")

local s = {
  status = "off",
  title = "—",
  artist = "",
  position = 0,
  duration = 0,
  rev = -1,
}

local function progress()
  if s.duration <= 0 then
    return 0
  end
  return math.min(1, s.position / s.duration)
end

local function render()
  panel.render(ui.column({ gap = 8, padding = 12, fill = "surface", radius = 10 }, {
    ui.label({ text = "Yandex Music · P0", color = "on_surface" }),
    ui.label({
      text = s.title .. (s.artist ~= "" and (" — " .. s.artist) or ""),
      color = "on_surface_variant",
      maxLines = 1,
    }),
    ui.progress({ progress = progress(), height = 2, fill = "primary", track = "surface_variant" }),
    ui.row({ gap = 6 }, {
      ui.button({ text = "Play/Pause", onClick = "toggle" }),
      ui.button({ text = "Prev", variant = "secondary", onClick = "prev" }),
      ui.button({ text = "Next", variant = "secondary", onClick = "next" }),
      ui.button({ text = "Скрыть", variant = "ghost", onClick = "hidePanel" }),
    }),
    ui.label({ text = "status: " .. s.status, color = "on_surface_variant", fontSize = 12 }),
  }))
end

local function apply(state)
  if type(state) ~= "table" or (state.rev or -1) <= s.rev then
    return
  end
  s.rev = state.rev or 0
  s.status = state.status or "idle"
  s.position = state.positionMs or 0
  if state.track then
    s.title = state.track.title or "—"
    s.duration = state.track.durationMs or 0
    s.artist = (state.track.artists and state.track.artists[1]) or ""
  end
end

function update()
  noctalia.setUpdateInterval(2000)
  ymd.get("/v1/state", function(state, err)
    if not err then
      apply(state)
    end
    render()
  end)
end

function onOpen(context)
  update()
end

function toggle()
  ymd.cmd("toggle", function() update() end)
end

function prev()
  ymd.cmd("prev", function() update() end)
end

function next()
  ymd.cmd("next", function() update() end)
end

function hidePanel()
  noctalia.togglePanel("pbolkhovitin/yandex-music:panel")
end
