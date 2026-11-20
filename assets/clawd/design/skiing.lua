-- Skiing: side view on red skis (drawn facing right; the engine turns it
-- at each end of the run), a low tuck with the pole trailing, a push. Snow
-- spray and tracks are the engine's.
local clawd = require("clawd")
local f = clawd.frame
local function ski(o)
  o = o or {}
  o.facing = "side"
  o.legs = "ski"
  o.crouch = o.crouch or 1
  o.lean = o.lean or 2
  o.eyes = o.eyes or "focus"
  o.arm_r = o.arm_r or { 3, 2 }
  o.hold = { "ski_pole", o.pole or -125 }
  o.pole = nil
  return o
end
return clawd.sheet({
  { "glide", {
    f(500, ski()),
    f(500, ski({ crouch = 2 })),
    f(500, ski()),
    f(160, ski({ eyes = "closed" })),
    f(500, ski({ eyes = "right", crouch = 2 })),
  } },
  -- One-shot: reach the pole forward, plant, push, back to the tuck.
  { "push", {
    f(200, ski({ arm_r = { 5, 0 }, pole = -70, lean = 1, crouch = 0 })),
    f(200, ski({ arm_r = { 4, 2 }, pole = -100, lean = 1, crouch = 0 })),
    f(250, ski({ arm_r = { 1, 3 }, pole = -150, crouch = 1 })),
    f(250, ski()),
  } },
}, { 30, 20, 14, 19 })
