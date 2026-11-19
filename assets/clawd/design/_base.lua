-- Shared base animations, used by any activity that doesn't draw its own.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  -- Breathe (a 1 px rise), then a blink.
  { "idle", {
    f(420, {}),
    f(420, { lift = 1 }),
    f(420, {}),
    f(420, { lift = 1 }),
    f(260, {}),
    f(110, { eyes = "closed" }),
    f(260, {}),
  } },
  -- Four-beat walk: alternate leg pairs lift, the body bobs.
  { "walk", {
    f(120, { legs = "walk_a", lift = 1 }),
    f(120, {}),
    f(120, { legs = "walk_b", lift = 1 }),
    f(120, {}),
  } },
  -- React set that activities can borrow.
  { "celebrate", {
    f(100, { arm_l = "up", arm_r = "up", lift = 1 }),
    f(100, { arm_l = "up", arm_r = "up", lift = 2, wiggle = true }),
    f(100, { arm_l = "up", arm_r = "up", lift = 1 }),
    f(140, {}),
  } },
  { "alert", {
    f(160, { eyes = "up", lift = 1 }),
    f(400, { eyes = "up" }),
  } },
})
