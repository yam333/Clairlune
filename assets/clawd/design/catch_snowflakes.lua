-- Catching snowflakes: face up, arms out, in a beanie and scarf; a hop
-- and a twirl when one lands.
local clawd = require("clawd")
local f = clawd.frame
local warm = { "beanie", "scarf" }
return clawd.sheet({
  { "catch", {
    f(600, { eyes = "up", arm_l = "out", arm_r = "out", costume = warm }),
    f(600, { eyes = "up", arm_l = "out", arm_r = "out", wiggle = true, costume = warm }),
    f(300, { eyes = "happy", arm_l = "out", arm_r = "out", lift = 1, costume = warm }),
  } },
  { "twirl", {
    f(250, { eyes = "happy", arm_l = "up", arm_r = "up", lift = 1, costume = warm }),
    f(250, { facing = "side", eyes = "up", arm_r = "up", lift = 1, costume = warm }),
    f(250, { eyes = "happy", arm_l = "up", arm_r = "up", costume = warm }),
  } },
}, { 24, 26, 11, 25 })
