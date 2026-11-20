-- Flying a broom: seated on the handle, both hands forward on it, the
-- wizard's hat on (drawn facing right; the engine turns it at each end of
-- the flight). The bristles' end is the item tip, where sparkles stream.
local clawd = require("clawd")
local f = clawd.frame
local function ride(o)
  o = o or {}
  o.facing = "side"
  o.sit = true
  o.costume = { "wizard_hat" }
  o.arm_r = o.arm_r or { 4, 3 }
  o.hold = { "broom", 184 }
  o.eyes = o.eyes or "right"
  return o
end
return clawd.sheet({
  { "fly", {
    f(500, ride()),
    f(500, ride({ lift = 1 })),
    f(500, ride()),
    f(160, ride({ eyes = "closed" })),
    f(500, ride()),
  } },
  { "glance", {
    f(600, ride({ eyes = "happy" })),
    f(600, ride({ eyes = "up" })),
  } },
}, { 34, 20, 20, 19 })
