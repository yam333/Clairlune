-- Playing the flute: held across below the eyes, both hands on it,
-- swaying with the tune; notes leave from the end past the right hand.
local clawd = require("clawd")
local f = clawd.frame
local function flute(o)
  o = o or {}
  o.hold = { "flute", 180 }
  o.arm_r = o.arm_r or { 1, 1 }
  o.arm_l = o.arm_l or { 3, 1 }
  return o
end
return clawd.sheet({
  { "play", {
    f(400, flute({ eyes = "closed" })),
    f(400, flute({ eyes = "closed", lean = 1 })),
    f(400, flute({ eyes = "happy", lift = 1 })),
    f(400, flute({ eyes = "closed", lean = -1 })),
  } },
  { "pause", {
    f(900, { eyes = "open", hold = { "flute", 200 }, arm_r = { -1, 4 }, arm_l = "rest" }),
    f(900, { eyes = "happy", hold = { "flute", 200 }, arm_r = { -1, 4 }, arm_l = "rest" }),
  } },
}, { 28, 22, 13, 21 })
