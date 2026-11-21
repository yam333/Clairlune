-- Blowing bubbles: the wand up to the mouth, then waved through the air;
-- the wand's ring is the item tip, where the bubbles come from.
local clawd = require("clawd")
local f = clawd.frame
local function blow(o)
  o = o or {}
  o.hold = { "bubble_wand", o.deg or 40 }
  o.deg = nil
  o.arm_r = o.arm_r or { 2, -1 }
  return o
end
return clawd.sheet({
  { "blow", {
    f(400, blow({ eyes = "happy" })),
    f(400, blow({ eyes = "closed", arm_r = { 2, -2 } })),
    f(400, blow({ eyes = "happy", lift = 1 })),
  } },
  { "wave", {
    f(250, blow({ eyes = "up", arm_r = "up", deg = 80 })),
    f(250, blow({ eyes = "up", arm_r = "out", deg = 20 })),
    f(250, blow({ eyes = "happy", arm_r = "up", deg = 80 })),
  } },
}, { 28, 24, 11, 23 })
