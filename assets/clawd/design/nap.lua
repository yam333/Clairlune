-- Dozing on the ground: slow breathing with eyes shut, a stretch, a sleepy peek.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "doze", {
    f(1500, { sit = true, eyes = "closed", arm_l = { -1, 3 }, arm_r = { 1, 3 } }),
    f(1500, { sit = true, eyes = "closed", arm_l = { -1, 3 }, arm_r = { 1, 3 }, lift = 1 }),
  } },
  -- One-shot: a slow sleepy stretch upward.
  { "yawn", {
    f(450, { sit = true, eyes = "closed", arm_l = "up", arm_r = "up" }),
    f(550, { sit = true, eyes = "happy", arm_l = "up", arm_r = "up", wiggle = true }),
    f(350, { sit = true, eyes = "closed", arm_l = { -1, 3 }, arm_r = { 1, 3 } }),
  } },
  -- One-shot: a half-awake glance before drifting back off.
  { "peek", {
    f(500, { sit = true, eyes = "open", arm_l = { -1, 3 }, arm_r = { 1, 3 } }),
    f(400, { sit = true, eyes = "happy", arm_l = { -1, 3 }, arm_r = { 1, 3 } }),
  } },
})
