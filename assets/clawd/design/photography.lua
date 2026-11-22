-- Taking photos of the view: the camera up at eye level, a click and a
-- flash, then a look at the picture.
local clawd = require("clawd")
local f = clawd.frame
local function aim(o)
  o = o or {}
  o.hold = { "camera" }
  o.arm_r = o.arm_r or { 0, -2 }
  o.eyes = o.eyes or "right"
  return o
end
local function look(o)
  o = o or {}
  o.hold = { "camera" }
  o.arm_r = o.arm_r or { -3, 3 }
  return o
end
return clawd.sheet({
  { "aim", {
    f(900, aim()),
    f(700, aim({ lean = 1 })),
    f(900, aim({ eyes = "focus" })),
  } },
  -- One-shot: the click.
  { "snap", {
    f(150, aim({ eyes = "closed", wiggle = true })),
    f(450, aim({ eyes = "focus" })),
  } },
  { "check", {
    f(900, look({ eyes = "focus" })),
    f(700, look({ eyes = "happy" })),
    f(500, look({ eyes = "happy", lift = 1 })),
  } },
}, { 28, 22, 11, 21 })
