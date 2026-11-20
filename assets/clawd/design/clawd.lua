-- The project's own Clawd, drawn from scratch as code (docs/clawd-style.md,
-- docs/activity-system.md). Fan art of Anthropic's character: not covered by the
-- code licence (NOTICE).
--
-- A rig: `M.draw(canvas, pose)` draws Clawd from a pose description and returns
-- the anchors. Everything is positioned relative to the origin (ox, oy): the body
-- centre column on the lowest foot row. Front view: body 15 px wide, centred on
-- ox. Side view (facing right; the engine mirrors for left): body 11 px wide.

local M = {}

M.color = {
  body = { 0xD9, 0x78, 0x5C },
  shadow = { 0xB8, 0x5E, 0x45 },
  light = { 0xE8, 0x95, 0x7A },
  eye = { 0x26, 0x1A, 0x16 },
}
local C = M.color

-- Shared prop / costume palette (scene budget: PLAN.md section 4.5).
M.prop = {
  wood = { 0x7A, 0x52, 0x34 },
  wood_dark = { 0x4E, 0x34, 0x22 },
  metal = { 0x5E, 0x6B, 0x7A },
  metal_dark = { 0x3B, 0x44, 0x50 },
  spark = { 0xFF, 0xE9, 0xA0 },
  white = { 0xEE, 0xEA, 0xE0 },
  ink = { 0x2A, 0x26, 0x24 },
  purple = { 0x5B, 0x4B, 0x8A },
  purple_dark = { 0x41, 0x35, 0x66 },
  brass = { 0xC9, 0xA4, 0x5C },
  red = { 0xC8, 0x45, 0x2E },
  red_dark = { 0x93, 0x30, 0x22 },
  blue = { 0x3B, 0x6E, 0xA8 },
  glass = { 0x7F, 0xB3, 0xD5 },
  green = { 0x5E, 0x7A, 0x45 },
  green_dark = { 0x40, 0x56, 0x2F },
  ball = { 0xD8, 0xE0, 0x4A },
}
local P = M.prop

local BODY_H = 10

-- ---------------------------------------------------------------- primitives

local function line(c, x0, y0, x1, y1, col)
  local dx, dy = math.abs(x1 - x0), -math.abs(y1 - y0)
  local sx, sy = x0 < x1 and 1 or -1, y0 < y1 and 1 or -1
  local err = dx + dy
  while true do
    c.px(x0, y0, col)
    if x0 == x1 and y0 == y1 then return end
    local e2 = 2 * err
    if e2 >= dy then err = err + dy; x0 = x0 + sx end
    if e2 <= dx then err = err + dx; y0 = y0 + sy end
  end
end
M.line = line

-- Point at `len` px from (x, y) along `deg` (0 = right, 90 = up).
local function along(x, y, deg, len)
  local r = math.rad(deg)
  return math.floor(x + math.cos(r) * len + 0.5), math.floor(y - math.sin(r) * len + 0.5)
end
M.along = along

-- ---------------------------------------------------------------- pose

-- Pose controls (all optional; see docs/activity-system.md):
--   facing   "front" | "side"
--   lift     body raised n px (legs stretch)      crouch  body lowered n px (legs bend)
--   lean     upper body shifted n px toward the facing direction
--   sit      body rests on the ground line; legs become feet stubs
--   eyes     "open" | "closed" | "left" | "right" | "up" | "happy" | "focus"
--   arm_l, arm_r   "rest" | "out" | "up" | { dx, dy } hand position from the shoulder
--   reach, wiggle  extra length / 1 px raise for "out" and "up" arms
--   hold     { item, angle, hand = "r" }  item: wand, racket, ski_pole, walking_stick, mug, chess_piece, rod, stone, broom
--   costume  { "wizard_hat", "beanie", "goggles", "backpack", "scarf" }
--   legs     "stand" | "walk_a" | "walk_b" | "ski"
function M.pose(o)
  o = o or {}
  return {
    facing = o.facing or "front",
    lift = o.lift or 0,
    crouch = o.crouch or 0,
    lean = o.lean or 0,
    sit = o.sit or false,
    eyes = o.eyes or "open",
    arm_l = o.arm_l or "rest",
    arm_r = o.arm_r or "rest",
    reach = o.reach or 2,
    wiggle = o.wiggle or false,
    hold = o.hold,
    costume = o.costume or {},
    legs = o.legs or "stand",
    extra = o.extra,
  }
end

local function has(list, x)
  for _, v in ipairs(list) do if v == x then return true end end
  return false
end

-- ---------------------------------------------------------------- items

-- Draws a held item from the hand at `deg`; returns the item tip.
local ITEMS = {}
function ITEMS.wand(c, x, y, deg)
  local tx, ty = along(x, y, deg, 7)
  line(c, x, y, tx, ty, P.wood)
  c.px(tx, ty, P.spark)
  return { tx, ty }
end
function ITEMS.walking_stick(c, x, y, deg)
  local ux, uy = along(x, y, deg + 180, 2)
  local tx, ty = along(x, y, deg, 10)
  line(c, ux, uy, tx, ty, P.wood)
  return { tx, ty }
end
function ITEMS.ski_pole(c, x, y, deg)
  local tx, ty = along(x, y, deg, 11)
  line(c, x, y, tx, ty, P.metal)
  local bx, by = along(x, y, deg, 9)
  c.px(bx - 1, by, P.metal_dark); c.px(bx + 1, by, P.metal_dark)
  return { tx, ty }
end
function ITEMS.racket(c, x, y, deg)
  local nx, ny = along(x, y, deg, 3)
  line(c, x, y, nx, ny, P.wood_dark)
  local cx, cy = along(x, y, deg, 6)
  -- Head: a small ring, 5 px across, around (cx, cy).
  for a = 0, 359, 15 do
    local px, py = along(cx, cy, a, 2.5)
    c.px(px, py, P.metal_dark)
  end
  c.px(cx, cy, P.white)
  return { cx, cy }
end
function ITEMS.mug(c, x, y)
  c.rect(x - 1, y - 2, 3, 3, P.white)
  c.px(x + 2, y - 1, P.white)
  c.px(x, y - 2, P.wood_dark) -- coffee
  return { x, y - 3 }
end
function ITEMS.book(c, x, y)
  -- An open book held at the hand: a red cover framing two pages, a dark
  -- spine, a short line of text on each page (lines, not dots, so it never
  -- reads as a face).
  c.rect(x - 4, y - 2, 9, 4, P.red)
  c.rect(x - 3, y - 2, 3, 3, P.white)
  c.rect(x + 1, y - 2, 3, 3, P.white)
  c.rect(x, y - 2, 1, 4, P.red_dark)
  c.rect(x - 3, y - 1, 2, 1, { 0xC8, 0xC2, 0xB0 })
  c.rect(x + 2, y - 1, 2, 1, { 0xC8, 0xC2, 0xB0 })
  return { x, y - 3 }
end
function ITEMS.guitar(c, x, y, deg)
  -- Body at the strumming hand, neck toward `deg`, headstock at the end.
  local nx, ny = along(x, y, deg, 9)
  line(c, x, y, nx, ny, P.wood_dark)
  c.rect(nx - 1, ny - 1, 2, 2, P.ink)
  c.rect(x - 2, y - 1, 5, 4, P.wood)
  c.rect(x - 1, y - 2, 3, 1, P.wood)
  c.rect(x - 1, y + 3, 3, 1, P.wood)
  c.px(x, y, P.ink) -- sound hole
  return { x, y - 3 }
end
function ITEMS.rod(c, x, y, deg)
  -- A fishing rod: a short dark grip behind the hand with the reel under
  -- it, a long thin pole ahead. The line itself is the engine's (a tether).
  local bx, by = along(x, y, deg + 180, 2)
  line(c, bx, by, x, y, P.wood_dark)
  c.px(bx, by + 1, P.metal_dark)
  local tx, ty = along(x, y, deg, 14)
  line(c, x, y, tx, ty, P.wood)
  return { tx, ty }
end
function ITEMS.stone(c, x, y)
  -- A flat skimming stone between the fingers.
  c.rect(x, y - 1, 2, 1, P.metal_dark)
  return { x, y - 1 }
end
function ITEMS.broom(c, x, y, deg)
  -- A broom ridden like a wizard's: the handle from just ahead of the hand
  -- back under the body, the bristles fanning out at the tail; the tail
  -- end is the tip (sparkles stream from it).
  local fx, fy = along(x, y, deg + 180, 3)
  local bx, by = along(x, y, deg, 15)
  line(c, fx, fy, bx, by, P.wood)
  for i = 0, 4 do
    local ex, ey = along(bx, by, deg - 18 + i * 9, 5)
    line(c, bx, by, ex, ey, i % 2 == 0 and P.brass or P.wood_dark)
  end
  return { along(bx, by, deg, 5) }
end
function ITEMS.chess_piece(c, x, y)
  c.px(x, y - 3, P.ink)
  c.rect(x - 1, y - 2, 3, 1, P.ink)
  c.px(x, y - 1, P.ink)
  c.rect(x - 1, y, 3, 1, P.ink)
  return { x, y - 3 }
end

-- ---------------------------------------------------------------- body parts

local function geometry(p, ox, oy)
  local half = p.facing == "side" and 5 or 7
  local legs_len = p.sit and 0 or math.max(1, 3 - p.crouch)
  local bottom = oy - legs_len - (p.sit and 1 or 0) - p.lift
  if p.legs == "ski" then bottom = bottom - 1 end
  local top = bottom - BODY_H + 1
  return { ox = ox, oy = oy, half = half, top = top, bottom = bottom }
end

-- Horizontal shift of row y for the lean (0 at the bottom, `lean` at the top).
local function shift(p, g, y)
  if p.lean == 0 then return 0 end
  return math.floor(p.lean * (g.bottom - y) / (BODY_H - 1) + 0.5)
end

local function body(c, p, g)
  local l, r = g.ox - g.half, g.ox + g.half
  for y = g.top, g.bottom do
    local s = shift(p, g, y)
    for x = l, r do
      local corner = (y == g.top or y == g.bottom) and (x == l or x == r)
      if not corner then
        local col = C.body
        if y == g.top then col = C.light
        elseif y == g.bottom or x == r then col = C.shadow
        end
        c.px(x + s, y, col)
      end
    end
  end
end

local function eyes(c, p, g)
  local xs = p.facing == "side" and { g.ox + 2 } or { g.ox - 4, g.ox + 3 }
  local dx, dy = 0, 0
  if p.eyes == "left" then dx = -1 elseif p.eyes == "right" then dx = 1 elseif p.eyes == "up" then dy = -1 end
  for _, x in ipairs(xs) do
    local s = shift(p, g, g.top + 3)
    local ex, ey = x + dx + s, g.top + 2 + dy
    if p.eyes == "closed" then
      c.rect(ex, g.top + 4, 2, 1, C.eye)
    elseif p.eyes == "happy" then -- ^ ^
      c.px(ex, g.top + 3, C.eye); c.px(ex + 1, g.top + 2, C.eye)
      if p.facing ~= "side" then c.px(ex + 2, g.top + 3, C.eye) end
    elseif p.eyes == "focus" then
      c.rect(ex, g.top + 3, 2, 2, C.eye)
    else
      c.rect(ex, ey, 2, 3, C.eye)
    end
  end
end

-- Draws one arm; returns the hand position.
local function arm(c, p, g, side)
  local kind = side < 0 and p.arm_l or p.arm_r
  -- Side view: the shoulder sits lower so a raised arm never crosses the eye.
  local y = g.top + (p.facing == "side" and 6 or 4)
  local s = shift(p, g, y)
  if p.facing == "side" then
    -- The near arm hangs at the front half of the body; the far arm is hidden.
    if side < 0 then return { g.ox - 1 + s, y + 1 } end
  end
  local near = (side < 0 and g.ox - g.half - 1 or g.ox + g.half + 1) + s
  if p.facing == "side" then near = g.ox + 1 + s end

  if type(kind) == "table" then
    local hx, hy = near + kind[1], y + kind[2]
    line(c, near, y, hx, hy, C.body)
    line(c, near, y + 1, hx, hy + 1, C.shadow)
    c.px(hx, hy, C.light)
    return { hx, hy }
  elseif kind == "rest" then
    if p.facing == "side" then
      c.px(near + 4, y, C.body); c.px(near + 5, y, C.body)
      c.px(near + 4, y + 1, C.shadow); c.px(near + 5, y + 1, C.shadow)
      return { near + 5, y }
    end
    for i = 0, 1 do
      c.px(near + side * i, y, C.body)
      c.px(near + side * i, y + 1, C.shadow)
    end
    return { near + side, y }
  elseif kind == "out" then
    local hy = y - 1 - (p.wiggle and 1 or 0)
    local len = 2 + p.reach
    for i = 0, len - 1 do
      local yy = (i < 2) and y or hy
      c.px(near + side * i, yy, C.body)
      c.px(near + side * i, yy + 1, C.shadow)
    end
    return { near + side * (len - 1), hy }
  else -- "up": a 2 px arm along the body side, raised above the top edge
    local tip = y - 6 - (p.wiggle and 1 or 0)
    for yy = tip, y do
      c.px(near, yy, C.body)
      c.px(near + side, yy, side > 0 and C.shadow or C.body)
    end
    c.px(near, tip, C.light)
    c.px(near + side, tip, C.light)
    return { near, tip }
  end
end

local LEGS_FRONT = { -6, -4, 4, 6 } -- outer, inner, inner, outer
local LEGS_SIDE = { -4, -2, 2, 4 } -- back pair, front pair

local function legs(c, p, g)
  if p.sit then
    -- Sitting: feet poke out under the body.
    local xs = p.facing == "side" and { g.ox + 3, g.ox + 5 } or { g.ox - 5, g.ox + 4 }
    for _, x in ipairs(xs) do c.rect(x, g.bottom + 1, 2, 1, C.shadow) end
    return
  end
  local xs = p.facing == "side" and LEGS_SIDE or LEGS_FRONT
  local last = g.oy - (p.legs == "ski" and 1 or 0)
  for i, dx in ipairs(xs) do
    local lifted = (p.legs == "walk_a" and (i == 1 or i == 3)) or (p.legs == "walk_b" and (i == 2 or i == 4))
    local col = (i == 2 or i == 3) and C.shadow or C.body
    if p.facing == "side" then col = (i <= 2) and C.shadow or C.body end
    for y = g.bottom + 1, last - (lifted and 1 or 0) do
      c.px(g.ox + dx, y, col)
    end
  end
  if p.legs == "ski" then
    -- Skis under the feet, tips turned up at the front.
    line(c, g.ox - 9, g.oy, g.ox + 9, g.oy, P.blue)
    c.px(g.ox + 10, g.oy - 1, P.blue)
  end
end

-- ---------------------------------------------------------------- costumes

local COSTUME = {}
function COSTUME.wizard_hat(c, p, g)
  local s = shift(p, g, g.top)
  local cx = g.ox + s
  local w = g.half + 2
  c.rect(cx - w, g.top - 1, 2 * w + 1, 1, P.purple_dark) -- brim
  for i = 0, 7 do
    local hw = math.max(0, math.floor((w - 1) * (7 - i) / 7 + 0.5))
    local lean = math.floor(i * i / 16) -- the tip droops toward the back
    c.rect(cx - hw - lean, g.top - 2 - i, 2 * hw + 1, 1, P.purple)
  end
  c.rect(cx - w + 1, g.top - 2, 2 * w - 1, 1, P.brass) -- band
  c.px(cx + 1, g.top - 5, P.spark) -- star
end
function COSTUME.beanie(c, p, g)
  local s = shift(p, g, g.top)
  local l, r = g.ox - g.half + 1 + s, g.ox + g.half - 1 + s
  c.rect(l, g.top - 2, r - l + 1, 2, P.red)
  c.rect(l, g.top - 1, r - l + 1, 1, P.red_dark)
  c.px(g.ox + s, g.top - 3, P.white) -- pompom
end
function COSTUME.goggles(c, p, g)
  local s = shift(p, g, g.top + 3)
  local l, r = g.ox - g.half + s, g.ox + g.half + s
  c.rect(l, g.top + 2, r - l + 1, 1, P.ink)
  if p.facing == "side" then
    c.rect(g.ox + 2 + s, g.top + 1, 3, 3, P.glass)
  else
    c.rect(g.ox - 5 + s, g.top + 1, 4, 3, P.glass)
    c.rect(g.ox + 2 + s, g.top + 1, 4, 3, P.glass)
  end
end
function COSTUME.backpack(c, p, g)
  if p.facing ~= "side" then return end
  local x = g.ox - g.half - 3 + shift(p, g, g.top + 4)
  c.rect(x, g.top + 2, 3, 6, P.green)
  c.rect(x, g.top + 2, 3, 1, P.green_dark)
  c.px(x + 1, g.top + 5, P.brass)
end
function COSTUME.scarf(c, p, g)
  local s = shift(p, g, g.top + 6)
  local l, r = g.ox - g.half + s, g.ox + g.half + s
  c.rect(l, g.top + 6, r - l + 1, 1, P.red)
  c.rect(l + 2, g.top + 7, 2, 3, P.red_dark)
end

-- ---------------------------------------------------------------- draw

-- Draws a pose with the origin at (ox, oy) and returns its anchors.
function M.draw(c, p, ox, oy)
  local g = geometry(p, ox, oy)
  if has(p.costume, "backpack") then COSTUME.backpack(c, p, g) end
  legs(c, p, g)
  body(c, p, g)
  local side = p.facing == "side"
  if not side then eyes(c, p, g) end
  local hl = arm(c, p, g, -1)
  local hr = arm(c, p, g, 1)
  if side then eyes(c, p, g) end -- in profile the eye stays on top of the near arm
  for _, piece in ipairs(p.costume) do
    if piece ~= "backpack" then COSTUME[piece](c, p, g) end
  end
  -- `feet` is omitted unless it differs from the origin (it defaults to origin).
  local anchors = {
    origin = { ox, oy },
    head = { ox + shift(p, g, g.top), g.top },
    hand_l = hl,
    hand_r = hr,
  }
  if p.hold then
    local item, deg, hand = p.hold[1], p.hold[2] or 0, p.hold.hand or "r"
    local h = hand == "l" and hl or hr
    anchors.item_tip = ITEMS[item](c, h[1], h[2], deg)
  end
  if p.extra then p.extra(c, anchors) end
  return anchors
end

-- ---------------------------------------------------------------- sheets

-- A frame entry: duration in ms plus pose options. Finalised by `M.sheet`.
function M.frame(ms, opts)
  return { ms = ms, opts = opts or {} }
end

-- Builds { frames, tags } from an ordered list of { name, { frame, ... } }.
-- `canvas` = { w, h, ox, oy } (default 24 x 20 with the origin at 11, 19).
function M.sheet(animations, canvas)
  canvas = canvas or { 24, 20, 11, 19 }
  local W, H, ox, oy = canvas[1], canvas[2], canvas[3], canvas[4]
  local frames, tags = {}, {}
  for _, anim in ipairs(animations) do
    local from = #frames + 1
    for _, fr in ipairs(anim[2]) do
      local p = M.pose(fr.opts)
      local sink = { px = function() end, rect = function() end }
      table.insert(frames, {
        duration = fr.ms,
        draw = function(c) M.draw(c, p, ox, oy) end,
        anchors = M.draw(sink, p, ox, oy),
      })
    end
    table.insert(tags, { name = anim[1], from = from, to = #frames })
  end
  return { width = W, height = H, frames = frames, tags = tags }
end

return M
