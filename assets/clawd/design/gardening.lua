-- Watering the flowers: the can tipped over the pots (drawn facing right;
-- the engine mirrors it to face them), water falling from the rose, then a
-- step back to admire them.
local clawd = require("clawd")
local f = clawd.frame
local function can(deg, o)
  o = o or {}
  o.hold = { "watering_can", deg }
  o.arm_r = o.arm_r or { 3, 0 }
  o.eyes = o.eyes or "right"
  return o
end
return clawd.sheet({
  { "water", {
    f(500, can(-30, { eyes = "focus" })),
    f(500, can(-40, { eyes = "focus", arm_r = { 3, 1 } })),
    f(500, can(-35, { eyes = "happy" })),
    f(500, can(-45, { eyes = "focus", arm_r = { 4, 1 } })),
  } },
  { "admire", {
    f(800, can(20, { eyes = "happy", arm_r = { 1, 3 } })),
    f(800, can(20, { eyes = "happy", arm_r = { 1, 3 }, lift = 1 })),
    f(600, can(20, { eyes = "right", arm_r = { 1, 3 } })),
  } },
}, { 30, 22, 11, 21 })
