-- Hot cocoa in the cold: sitting in a beanie and scarf with a steaming
-- mug, taking sips. The mug's top is the item tip the steam rises from.
local clawd = require("clawd")
local f = clawd.frame
local function cup(o)
  o = o or {}
  o.sit = true
  o.hold = { "mug" }
  o.costume = { "beanie", "scarf" }
  o.arm_r = o.arm_r or { -3, 3 }
  return o
end
return clawd.sheet({
  { "hold", {
    f(700, cup({ eyes = "happy" })),
    f(700, cup({ eyes = "closed" })),
  } },
  { "sip", {
    f(400, cup({ eyes = "closed", arm_r = { -3, 1 } })),
    f(600, cup({ eyes = "closed", arm_r = { -3, 0 } })),
    f(400, cup({ eyes = "happy", arm_r = { -3, 2 } })),
  } },
})
