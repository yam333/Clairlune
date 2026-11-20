-- Hiking a ridge: side view, walking (the engine keeps Clawd in place while
-- the land goes by under the feet), a pack on the back, a beanie and a
-- walking stick that swings with the stride.
local clawd = require("clawd")
local f = clawd.frame
local function hike(o)
  o = o or {}
  o.facing = "side"
  o.lean = 1
  o.costume = { "backpack", "beanie" }
  o.eyes = o.eyes or "right"
  o.arm_r = o.arm_r or { 3, 2 }
  o.hold = { "walking_stick", o.stick or -75 }
  o.stick = nil
  return o
end
return clawd.sheet({
  { "hike", {
    f(140, hike({ legs = "walk_a", lift = 1, stick = -68 })),
    f(140, hike({ stick = -74 })),
    f(140, hike({ legs = "walk_b", lift = 1, stick = -84, arm_r = { 2, 2 } })),
    f(140, hike({ stick = -78 })),
  } },
  -- One-shot: eyes up to the peaks for a few steps, then a happy look.
  { "look", {
    f(140, hike({ legs = "walk_a", lift = 1, stick = -68, eyes = "up" })),
    f(140, hike({ stick = -74, eyes = "up" })),
    f(140, hike({ legs = "walk_b", lift = 1, stick = -84, arm_r = { 2, 2 }, eyes = "up" })),
    f(140, hike({ stick = -78, eyes = "up" })),
    f(140, hike({ legs = "walk_a", lift = 1, stick = -68, eyes = "happy" })),
    f(140, hike({ stick = -74, eyes = "happy" })),
    f(140, hike({ legs = "walk_b", lift = 1, stick = -84, arm_r = { 2, 2 } })),
    f(140, hike({ stick = -78 })),
  } },
}, { 28, 24, 13, 23 })
