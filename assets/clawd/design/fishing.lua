-- Fishing off the end of the pier, sitting, the rod held out over the water
-- (drawn facing right; the engine mirrors it to face the open water). The
-- line and the float are the engine's tether.
local clawd = require("clawd")
local f = clawd.frame
local function rod(deg, o)
  o = o or {}
  o.sit = true
  o.arm_r = o.arm_r or { 2, 1 }
  o.hold = { "rod", deg }
  o.eyes = o.eyes or "open"
  return o
end
return clawd.sheet({
  { "wait", {
    f(1400, rod(38)),
    f(160, rod(38, { eyes = "closed" })),
    f(1400, rod(38)),
    f(1000, rod(38, { eyes = "right" })),
  } },
  { "look", {
    f(900, rod(38, { eyes = "left" })),
    f(900, rod(38, { eyes = "up" })),
  } },
  -- One-shot: something nibbles; the rod tip twitches.
  { "nibble", {
    f(220, rod(32, { eyes = "focus" })),
    f(220, rod(38, { eyes = "focus" })),
    f(220, rod(30, { eyes = "focus" })),
    f(300, rod(38, { eyes = "focus" })),
  } },
  -- One-shot: a tug back, pleased, then settling again.
  { "reel", {
    f(250, rod(55, { eyes = "happy", arm_r = { 2, 0 } })),
    f(300, rod(62, { eyes = "happy", arm_r = { 2, -1 }, lift = 1 })),
    f(400, rod(50, { eyes = "happy", arm_r = { 2, 0 } })),
    f(300, rod(38)),
  } },
}, { 36, 20, 11, 19 })
