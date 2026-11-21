-- Sledding: seated on a wooden sled (side view, facing the way it goes),
-- in a beanie and a scarf, one hand on the front of the sled; bumps lift
-- it a pixel; now and then both arms go up. The sled is drawn under the
-- pose, its runner on the origin row.
local clawd = require("clawd")
local f = clawd.frame
local P = clawd.prop

local function sled(c, ox, oy)
  c.rect(ox - 9, oy - 2, 19, 1, P.wood)
  c.rect(ox - 9, oy - 1, 19, 1, P.wood_dark)
  clawd.line(c, ox - 10, oy, ox + 10, oy, P.metal_dark)
  -- The runner curls up at the front.
  c.px(ox + 11, oy - 1, P.metal_dark)
  c.px(ox + 12, oy - 2, P.metal_dark)
  c.px(ox + 12, oy - 3, P.metal_dark)
  c.px(ox + 11, oy - 4, P.metal_dark)
end

local function ride(o)
  o = o or {}
  o.facing = "side"
  o.sit = true
  o.lift = 2 + (o.bump or 0)
  o.bump = nil
  o.under = sled
  o.costume = { "beanie", "scarf" }
  o.eyes = o.eyes or "right"
  o.arm_r = o.arm_r or { 4, 2 }
  return o
end

return clawd.sheet({
  { "ride", {
    f(300, ride()),
    f(160, ride({ bump = 1 })),
    f(300, ride({ eyes = "happy" })),
    f(400, ride()),
    f(160, ride({ bump = 1, eyes = "happy" })),
    f(300, ride()),
  } },
  -- One-shot: both arms up, then a hand back on the sled.
  { "whee", {
    f(160, ride({ arm_r = "up", arm_l = "up", eyes = "happy" })),
    f(200, ride({ arm_r = "up", arm_l = "up", eyes = "happy", wiggle = true, bump = 1 })),
    f(200, ride({ arm_r = "up", arm_l = "up", eyes = "happy" })),
    f(200, ride({ arm_r = "up", arm_l = "up", eyes = "happy", wiggle = true })),
    f(200, ride()),
  } },
}, { 32, 24, 14, 23 })
