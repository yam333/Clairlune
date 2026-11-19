-- Coffee from a camp stove: holding a steaming mug, taking sips.
-- The mug's top is the `item_tip` anchor that the steam rises from.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "hold", {
    f(500, { eyes = "happy", arm_r = { -3, 3 }, hold = { "mug" } }),
    f(500, { eyes = "happy", arm_r = { -3, 3 }, hold = { "mug" }, lift = 1 }),
  } },
  { "sip", {
    f(300, { eyes = "closed", arm_r = { -3, 1 }, hold = { "mug" } }),
    f(500, { eyes = "closed", arm_r = { -3, 0 }, hold = { "mug" } }),
    f(300, { eyes = "happy", arm_r = { -3, 2 }, hold = { "mug" } }),
  } },
})
