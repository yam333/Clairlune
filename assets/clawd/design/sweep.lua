-- Sweeping with the broom: side view, the bristles swishing low in front;
-- the bristles' end is the item tip, where the dust rises.
local clawd = require("clawd")
local f = clawd.frame
local function sweep(o)
  o = o or {}
  o.facing = "side"
  o.lean = 1
  o.hold = { "broom", o.deg or -15 }
  o.deg = nil
  o.arm_r = o.arm_r or { 3, 1 }
  o.eyes = o.eyes or "right"
  return o
end
return clawd.sheet({
  { "sweep", {
    f(260, sweep({ deg = -12 })),
    f(260, sweep({ deg = -20, arm_r = { 4, 2 } })),
    f(260, sweep({ deg = -12, legs = "walk_a" })),
    f(260, sweep({ deg = -20, arm_r = { 4, 2 }, legs = "walk_b" })),
  } },
  { "rest", {
    f(800, sweep({ deg = -60, arm_r = { 2, 1 }, lean = 0, eyes = "happy" })),
    f(800, sweep({ deg = -60, arm_r = { 2, 1 }, lean = 0, eyes = "open" })),
  } },
}, { 38, 24, 11, 23 })
