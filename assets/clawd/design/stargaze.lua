-- Stargazing beside a telescope (drawn on the right; the engine mirrors it).
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "look_up", {
    f(600, { eyes = "up" }),
    f(600, { eyes = "up", lift = 1 }),
  } },
  { "adjust", {
    f(200, { eyes = "right", arm_r = "out" }),
    f(200, { eyes = "right", arm_r = "out", wiggle = true }),
    f(200, { eyes = "right", arm_r = "out" }),
    f(200, { eyes = "up" }),
  } },
})
