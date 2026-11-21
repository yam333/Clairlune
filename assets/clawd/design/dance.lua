-- Dancing under the evening sky: arms up and out in turn, little hops, a
-- spin; notes rise from a hummed tune (from the head).
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "dance", {
    f(220, { eyes = "happy", arm_l = "up", arm_r = "out", lift = 1 }),
    f(220, { eyes = "happy", arm_l = "out", arm_r = "up", legs = "walk_a" }),
    f(220, { eyes = "closed", arm_l = "up", arm_r = "up", lift = 2, wiggle = true }),
    f(220, { eyes = "happy", arm_l = "out", arm_r = "out", legs = "walk_b" }),
  } },
  { "spin", {
    f(180, { facing = "side", eyes = "right", arm_r = "out", lift = 1 }),
    f(180, { eyes = "happy", arm_l = "out", arm_r = "out" }),
    f(180, { facing = "side", eyes = "right", arm_r = "up", lift = 1 }),
    f(180, { eyes = "happy", arm_l = "up", arm_r = "up" }),
  } },
}, { 24, 26, 11, 25 })
