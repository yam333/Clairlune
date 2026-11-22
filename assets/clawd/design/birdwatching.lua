-- Birdwatching: binoculars up to the eyes, scanning the sky, then lowered
-- to point one out.
local clawd = require("clawd")
local f = clawd.frame
local function watch(o)
  o = o or {}
  o.hold = { "binoculars" }
  o.arm_r = o.arm_r or { -3, -1 }
  o.arm_l = o.arm_l or { 3, -1 }
  o.eyes = o.eyes or "focus"
  return o
end
return clawd.sheet({
  { "watch", {
    f(1000, watch()),
    f(900, watch({ lean = 1 })),
    f(1000, watch()),
    f(900, watch({ lean = -1 })),
  } },
  { "point", {
    f(700, { eyes = "up", arm_l = "up", hold = { "binoculars" }, arm_r = { -3, 6 } }),
    f(700, { eyes = "happy", arm_l = "up", wiggle = true, hold = { "binoculars" }, arm_r = { -3, 6 } }),
    f(600, { eyes = "up", arm_l = "up", hold = { "binoculars" }, arm_r = { -3, 6 } }),
  } },
}, { 26, 22, 12, 21 })
