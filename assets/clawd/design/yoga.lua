-- Yoga by a rolled mat: a slow reach up, side stretches, a seated rest.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "reach", {
    f(1200, { eyes = "closed", arm_l = "up", arm_r = "up" }),
    f(1200, { eyes = "closed", arm_l = "up", arm_r = "up", lift = 1, wiggle = true }),
  } },
  { "stretch", {
    f(1100, { eyes = "closed", arm_l = "up", arm_r = "out", lean = 1 }),
    f(500, { eyes = "closed", arm_l = "up", arm_r = "up" }),
    f(1100, { eyes = "closed", arm_l = "out", arm_r = "up", lean = -1 }),
  } },
  { "rest", {
    f(1500, { sit = true, eyes = "closed", arm_l = { -2, 3 }, arm_r = { 2, 3 } }),
    f(1500, { sit = true, eyes = "happy", arm_l = { -2, 3 }, arm_r = { 2, 3 } }),
  } },
}, { 24, 22, 11, 21 })
