-- Skipping stones: a stone in hand, a wind-up and a sidearm throw toward the
-- water, then watching it skip (drawn facing right; the engine mirrors it to
-- face the open water). The stone's flight is the engine's projectile, from
-- the throwing hand once it's empty.
local clawd = require("clawd")
local f = clawd.frame
local stone = { "stone" }
return clawd.sheet({
  { "pick", {
    f(900, { eyes = "right", arm_r = { 1, 3 }, hold = stone }),
    f(160, { eyes = "closed", arm_r = { 1, 3 }, hold = stone }),
    f(900, { eyes = "right", arm_r = { 1, 3 }, hold = stone }),
    f(800, { eyes = "open", arm_r = { 1, 3 }, hold = stone }),
  } },
  -- One-shot: crouch and draw back, then the sidearm throw.
  { "throw", {
    f(200, { eyes = "focus", crouch = 1, arm_r = { -3, 2 }, hold = stone }),
    f(150, { eyes = "focus", crouch = 1, lean = -1, arm_r = { -4, 1 }, hold = stone }),
    f(120, { eyes = "focus", arm_r = "out", reach = 2 }),
    f(200, { eyes = "right", arm_r = "out", reach = 3 }),
  } },
  { "watch", {
    f(700, { eyes = "right", arm_r = "out", reach = 2 }),
    f(900, { eyes = "right", arm_r = "rest" }),
    f(900, { eyes = "happy", arm_r = "rest" }),
    f(900, { eyes = "right", arm_r = "rest" }),
  } },
})
