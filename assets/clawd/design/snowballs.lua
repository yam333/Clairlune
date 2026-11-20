-- Snowballs: packing one in both hands, an overarm throw toward the
-- snowman (drawn facing right; the engine mirrors it to face the snowman),
-- then watching it hit. The flight is the engine's projectile.
local clawd = require("clawd")
local f = clawd.frame
local ball = { "snowball" }
return clawd.sheet({
  { "pack", {
    f(300, { eyes = "focus", arm_r = { -1, 4 }, arm_l = { 1, 4 }, hold = ball }),
    f(300, { eyes = "focus", arm_r = { -1, 3 }, arm_l = { 1, 3 }, hold = ball }),
    f(300, { eyes = "focus", arm_r = { -1, 4 }, arm_l = { 1, 4 }, hold = ball }),
    f(500, { eyes = "right", arm_r = { -1, 4 }, arm_l = { 1, 4 }, hold = ball }),
  } },
  -- One-shot: arm back and up, then the throw.
  { "throw", {
    f(200, { eyes = "focus", arm_r = "up", hold = ball, lean = -1 }),
    f(120, { eyes = "focus", arm_r = "out", reach = 2 }),
    f(180, { eyes = "right", arm_r = "out", reach = 3 }),
  } },
  { "watch", {
    f(500, { eyes = "right", arm_r = "out", reach = 2 }),
    f(700, { eyes = "happy", arm_r = "rest" }),
    f(700, { eyes = "right", arm_r = "rest" }),
  } },
})
