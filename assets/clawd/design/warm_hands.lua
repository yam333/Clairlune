-- Warming hands at a campfire. Drawn with the fire on the right; the engine
-- mirrors the sprite when the fire is on the left.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  -- Hand stretched toward the fire, fingers wiggling.
  { "warm", {
    f(300, { arm_r = "out", eyes = "right" }),
    f(300, { arm_r = "out", eyes = "right", wiggle = true }),
    f(300, { arm_r = "out", eyes = "right" }),
    f(300, { arm_r = "out", eyes = "right", wiggle = true, lift = 1 }),
  } },
  -- A glance left and right, back to centre (one-shot).
  { "look_around", {
    f(320, { eyes = "left" }),
    f(320, { eyes = "left" }),
    f(110, { eyes = "closed" }),
    f(320, { eyes = "right" }),
    f(320, { eyes = "right" }),
    f(200, {}),
  } },
})
