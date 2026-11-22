-- Reads anchor marker pixels from the `anchors` layer of a sprite and writes
-- them as per-frame slice keys (the shape of Aseprite's own slice export), so
-- the sprite loader can read them like slices. Aseprite's Lua API can't set
-- per-frame slice keys, which is why anchors are markers (docs/dev/clawd-style.md).
--
-- Usage:
--   aseprite -b --script-param src=<file.aseprite> --script-param out=<anchors.json> \
--            --script scripts/aseprite/anchors.lua

local src = app.params["src"]
local out = app.params["out"]
if not src or not out then
  error("anchors.lua needs --script-param src=... and --script-param out=...")
end

local MARKERS = {
  ["255,0,0"] = "origin",
  ["0,255,0"] = "feet",
  ["0,0,255"] = "head",
  ["255,255,0"] = "hand_l",
  ["0,255,255"] = "hand_r",
  ["255,0,255"] = "item_tip",
}
local ORDER = { "origin", "feet", "head", "hand_l", "hand_r", "item_tip" }

local sprite = app.open(src)
if not sprite then error("can't open " .. src) end

local layer = nil
for _, l in ipairs(sprite.layers) do
  if l.name == "anchors" then layer = l end
end

local pc = app.pixelColor
local keys = {}
if layer then
  for i, frame in ipairs(sprite.frames) do
    local cel = layer:cel(frame.frameNumber)
    if cel then
      for it in cel.image:pixels() do
        local px = it()
        if pc.rgbaA(px) == 255 then
          local k = pc.rgbaR(px) .. "," .. pc.rgbaG(px) .. "," .. pc.rgbaB(px)
          local name = MARKERS[k]
          if name then
            keys[name] = keys[name] or {}
            table.insert(keys[name], { i - 1, it.x + cel.position.x, it.y + cel.position.y })
          end
        end
      end
    end
  end
end

local parts = {}
for _, name in ipairs(ORDER) do
  local list = keys[name]
  if list then
    local ks = {}
    for _, k in ipairs(list) do
      table.insert(ks, string.format('{"frame":%d,"bounds":{"x":%d,"y":%d,"w":1,"h":1}}', k[1], k[2], k[3]))
    end
    table.insert(parts, string.format('{"name":"%s","keys":[%s]}', name, table.concat(ks, ",")))
  end
end

local f = assert(io.open(out, "w"))
f:write('{"slices":[' .. table.concat(parts, ",") .. "]}\n")
f:close()
sprite:close()
