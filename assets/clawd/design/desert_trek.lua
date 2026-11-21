-- A trek over the dunes: side view, walking (the dunes go by under the
-- feet), goggles and a scarf against the sand, a walking stick.
local clawd = require("clawd")
local f = clawd.frame
local function walk(o)
  o = o or {}
  o.facing = "side"
  o.lean = 1
  o.costume = { "goggles", "scarf" }
  o.eyes = o.eyes or "right"
  o.arm_r = o.arm_r or { 3, 2 }
  o.hold = { "walking_stick", o.stick or -75 }
  o.stick = nil
  return o
end
return clawd.sheet({
  { "walk", {
    f(140, walk({ legs = "walk_a", lift = 1, stick = -68 })),
    f(140, walk({ stick = -74 })),
    f(140, walk({ legs = "walk_b", lift = 1, stick = -84, arm_r = { 2, 2 } })),
    f(140, walk({ stick = -78 })),
  } },
  -- One-shot: eyes up to the mesas for a few steps.
  { "look", {
    f(140, walk({ legs = "walk_a", lift = 1, stick = -68, eyes = "up" })),
    f(140, walk({ stick = -74, eyes = "up" })),
    f(140, walk({ legs = "walk_b", lift = 1, stick = -84, arm_r = { 2, 2 }, eyes = "up" })),
    f(140, walk({ stick = -78, eyes = "up" })),
    f(140, walk({ legs = "walk_a", lift = 1, stick = -68, eyes = "happy" })),
    f(140, walk({ stick = -74, eyes = "happy" })),
    f(140, walk({ legs = "walk_b", lift = 1, stick = -84, arm_r = { 2, 2 } })),
    f(140, walk({ stick = -78 })),
  } },
}, { 28, 22, 13, 21 })
