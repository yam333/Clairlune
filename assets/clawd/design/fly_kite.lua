-- Flying a kite: the string hand up, eyes on the kite (drawn facing right;
-- the engine mirrors it to face the open sky). The string and the kite are
-- the engine's tether, from the raised hand.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "fly", {
    f(800, { eyes = "up", arm_r = "up" }),
    f(700, { eyes = "up", arm_r = "up", wiggle = true }),
    f(900, { eyes = "happy", arm_r = "up" }),
    f(160, { eyes = "closed", arm_r = "up" }),
    f(800, { eyes = "up", arm_r = "up" }),
  } },
  { "look", {
    f(900, { eyes = "left", arm_r = "up" }),
    f(900, { eyes = "right", arm_r = "up" }),
  } },
  -- One-shot: two tugs, leaning back a little.
  { "tug", {
    f(220, { eyes = "focus", arm_r = "up", wiggle = true, lean = -1 }),
    f(220, { eyes = "focus", arm_r = "up" }),
    f(220, { eyes = "focus", arm_r = "up", wiggle = true, lean = -1 }),
    f(300, { eyes = "happy", arm_r = "up" }),
  } },
})
