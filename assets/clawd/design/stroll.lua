-- Strolling with a lantern. `walk` comes from the base sheet; this adds the wave.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "wave", {
    f(140, { arm_r = "up", eyes = "happy" }),
    f(140, { arm_r = "up", eyes = "happy", wiggle = true }),
    f(140, { arm_r = "up", eyes = "happy" }),
    f(140, { arm_r = "up", eyes = "happy", wiggle = true }),
    f(160, {}),
  } },
})
