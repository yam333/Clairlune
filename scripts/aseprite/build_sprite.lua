-- Builds an .aseprite source file from a design file in assets/clawd/design/.
-- The art is drawn by code, so the drawing history is reviewable in git.
--
-- Usage:
--   aseprite -b --script-param root=<repo root> \
--            --script-param def=assets/clawd/design/_base.lua \
--            --script-param out=assets/clawd/src/_base.aseprite \
--            --script scripts/aseprite/build_sprite.lua
--
-- A design file returns:
--   { width, height,
--     frames = { { duration = <ms>, draw = function(canvas) ... end,
--                  anchors = { origin = {x, y}, hand_r = {x, y}, ... } }, ... },
--     tags = { { name = "idle", from = 1, to = 4 }, ... } }   -- 1-based, inclusive

local root = app.params["root"]
local def_path = app.params["def"]
local out = app.params["out"]
if not (root and def_path and out) then
  error("build_sprite.lua needs --script-param root=, def= and out=")
end
package.path = root .. "/assets/clawd/design/?.lua;" .. package.path
local def = dofile(root .. "/" .. def_path)

local pc = app.pixelColor
local MARKERS = {
  origin = pc.rgba(255, 0, 0, 255),
  feet = pc.rgba(0, 255, 0, 255),
  head = pc.rgba(0, 0, 255, 255),
  hand_l = pc.rgba(255, 255, 0, 255),
  hand_r = pc.rgba(0, 255, 255, 255),
  item_tip = pc.rgba(255, 0, 255, 255),
}
local GUIDE = pc.rgba(80, 160, 255, 90)

local W, H = def.width, def.height
local sprite = Sprite(W, H, ColorMode.RGB)
local body = sprite.layers[1]
body.name = "body"
local anchors = sprite:newLayer()
anchors.name = "anchors"
anchors.isVisible = false
local guides = sprite:newLayer()
guides.name = "guides"
guides.isVisible = false

for i = 2, #def.frames do
  sprite:newEmptyFrame(i)
end

local function set_cel(layer, frame, image)
  local cel = layer:cel(frame)
  if cel then
    cel.image = image
    cel.position = Point(0, 0)
  else
    sprite:newCel(layer, frame, image, Point(0, 0))
  end
end

for i, fr in ipairs(def.frames) do
  sprite.frames[i].duration = fr.duration / 1000

  local img = Image(W, H, ColorMode.RGB)
  local canvas = {}
  function canvas.px(x, y, rgb)
    if x >= 0 and y >= 0 and x < W and y < H then
      img:drawPixel(x, y, pc.rgba(rgb[1], rgb[2], rgb[3], 255))
    end
  end
  function canvas.rect(x, y, w, h, rgb)
    for yy = y, y + h - 1 do
      for xx = x, x + w - 1 do
        canvas.px(xx, yy, rgb)
      end
    end
  end
  fr.draw(canvas)
  set_cel(body, i, img)

  local a = Image(W, H, ColorMode.RGB)
  for name, p in pairs(fr.anchors) do
    local m = MARKERS[name]
    if not m then error("unknown anchor " .. name) end
    a:drawPixel(p[1], p[2], m)
  end
  set_cel(anchors, i, a)

  local g = Image(W, H, ColorMode.RGB)
  local o = fr.anchors.origin
  for x = 0, W - 1 do g:drawPixel(x, o[2], GUIDE) end
  for y = 0, H - 1 do g:drawPixel(o[1], y, GUIDE) end
  set_cel(guides, i, g)
end

for _, t in ipairs(def.tags) do
  local tag = sprite:newTag(t.from, t.to)
  tag.name = t.name
end

sprite:saveAs(root .. "/" .. out)
sprite:close()
print("built " .. out .. " (" .. #def.frames .. " frames, " .. #def.tags .. " tags)")
