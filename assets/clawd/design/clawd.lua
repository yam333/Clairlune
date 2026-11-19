-- The project's own Clawd, drawn from scratch as code (docs/clawd-style.md).
-- Fan art of Anthropic's character: not covered by the code licence (NOTICE).
--
-- Canvas 24 x 20. The body is 15 px wide (columns 4..18), centred on column 11,
-- so every pose is symmetric about the origin column. The feet stand on row 19.

local M = {}

M.W, M.H = 24, 20
M.CENTER, M.BASELINE = 11, 19

M.color = {
  body = { 0xD9, 0x78, 0x5C },
  shadow = { 0xB8, 0x5E, 0x45 },
  light = { 0xE8, 0x95, 0x7A },
  eye = { 0x26, 0x1A, 0x16 },
}
local C = M.color

local BODY_L, BODY_R = 4, 18 -- inclusive columns
local BODY_H = 10
local LEGS = { 5, 7, 15, 17 } -- outer, inner, inner, outer

-- Pose options (all optional):
--   lift      body raised by n px; legs stretch to keep the feet on the ground
--   eyes      "open" | "closed" | "left" | "right" | "up"
--   arm_l/r   "rest" | "out" (reaching sideways) | "up" (raised)
--   reach     extra px for an "out" arm (default 2)
--   wiggle    raise "out" hands by 1 px (warming, waving)
--   legs      "stand" | "walk_a" | "walk_b"
function M.pose(o)
  o = o or {}
  local lift = o.lift or 0
  local top = M.BASELINE - 3 - BODY_H + 1 - lift -- 7 when lift = 0
  local bottom = top + BODY_H - 1
  return {
    lift = lift,
    top = top,
    bottom = bottom,
    eyes = o.eyes or "open",
    arm_l = o.arm_l or "rest",
    arm_r = o.arm_r or "rest",
    reach = o.reach or 2,
    wiggle = o.wiggle or false,
    legs = o.legs or "stand",
  }
end

local function arm(c, p, side)
  local kind = side < 0 and p.arm_l or p.arm_r
  local y = p.top + 4
  local near = side < 0 and BODY_L - 1 or BODY_R + 1
  if kind == "rest" then
    -- Two-pixel nub, shadow underneath.
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

local function eyes(c, p)
  local ex = { 7, 14 }
  local dx, dy, h = 0, 0, 3
  if p.eyes == "left" then dx = -1
  elseif p.eyes == "right" then dx = 1
  elseif p.eyes == "up" then dy = -1
  end
  for _, x in ipairs(ex) do
    if p.eyes == "closed" then
      c.rect(x + dx, p.top + 4, 2, 1, C.eye)
    else
      c.rect(x + dx, p.top + 2 + dy, 2, h, C.eye)
    end
  end
end

local function legs(c, p)
  for i, x in ipairs(LEGS) do
    local lifted = (p.legs == "walk_a" and (i == 1 or i == 3)) or (p.legs == "walk_b" and (i == 2 or i == 4))
    local last = M.BASELINE - (lifted and 1 or 0)
    local col = (i == 2 or i == 3) and C.shadow or C.body
    for y = p.bottom + 1, last do
      c.px(x, y, col)
    end
  end
end

-- Draws a pose and returns its anchors.
function M.draw(c, p)
  legs(c, p)
  -- Body: rounded corners, light top edge, shadow on the right and bottom.
  for y = p.top, p.bottom do
    for x = BODY_L, BODY_R do
      local corner = (y == p.top or y == p.bottom) and (x == BODY_L or x == BODY_R)
      if not corner then
        local col = C.body
        if y == p.top then
          col = C.light
        elseif y == p.bottom or x == BODY_R then
          col = C.shadow
        end
        c.px(x, y, col)
      end
    end
  end
  eyes(c, p)
  local hl = arm(c, p, -1)
  local hr = arm(c, p, 1)
  -- `feet` is omitted: it would share the origin's pixel, and defaults to origin.
  return {
    origin = { M.CENTER, M.BASELINE },
    head = { M.CENTER, p.top },
    hand_l = hl,
    hand_r = hr,
  }
end

-- A frame entry for a design file: duration in ms plus a pose.
function M.frame(ms, opts)
  local p = M.pose(opts)
  return {
    duration = ms,
    draw = function(c)
      M.draw(c, p)
    end,
    -- Anchors are computed by drawing once into a throwaway canvas.
    anchors = (function()
      local sink = { px = function() end, rect = function() end }
      return M.draw(sink, p)
    end)(),
  }
end

-- Builds { frames, tags } from an ordered list of { name, { frame, ... } }.
function M.sheet(animations)
  local frames, tags = {}, {}
  for _, anim in ipairs(animations) do
    local from = #frames + 1
    for _, f in ipairs(anim[2]) do
      table.insert(frames, f)
    end
    table.insert(tags, { name = anim[1], from = from, to = #frames })
  end
  return { width = M.W, height = M.H, frames = frames, tags = tags }
end

return M
