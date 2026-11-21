-- Meditating by a cairn: sitting still, eyes closed, breathing slowly,
-- now and then a peek and a smile.
local clawd = require("clawd")
local f = clawd.frame
local function sit(o)
  o = o or {}
  o.sit = true
  o.arm_l = o.arm_l or { -2, 3 }
  o.arm_r = o.arm_r or { 2, 3 }
  return o
end
return clawd.sheet({
  { "breathe", {
    f(1400, sit({ eyes = "closed" })),
    f(1400, sit({ eyes = "closed", lift = 1, arm_l = { -2, 2 }, arm_r = { 2, 2 } })),
  } },
  { "peek", {
    f(700, sit({ eyes = "open" })),
    f(500, sit({ eyes = "happy" })),
  } },
})
