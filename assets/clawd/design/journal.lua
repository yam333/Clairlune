-- Writing in a notebook held on the lap, pausing to look out at the scene.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "write", {
    f(550, { sit = true, eyes = "focus", arm_l = { 4, 4 }, arm_r = { -7, 5 }, hold = { "book" } }),
    f(550, { sit = true, eyes = "focus", arm_l = { 5, 3 }, arm_r = { -8, 5 }, hold = { "book" } }),
  } },
  { "ponder", {
    f(1100, { sit = true, eyes = "up", arm_l = { 2, 3 }, arm_r = { -8, 5 }, hold = { "book" } }),
    f(1100, { sit = true, eyes = "left", arm_l = { 2, 3 }, arm_r = { -8, 5 }, hold = { "book" } }),
  } },
  -- One-shot: pleased with the line just written.
  { "nod", {
    f(350, { sit = true, eyes = "happy", arm_l = { 4, 4 }, arm_r = { -8, 5 }, hold = { "book" }, lift = 1 }),
    f(350, { sit = true, eyes = "happy", arm_l = { 4, 4 }, arm_r = { -8, 5 }, hold = { "book" } }),
  } },
})
