-- Reading by lantern light, sitting with an open book in both hands.
local clawd = require("clawd")
local f = clawd.frame
return clawd.sheet({
  { "read", {
    f(900, { sit = true, eyes = "focus", arm_r = { -8, 5 }, hold = { "book" } }),
    f(900, { sit = true, eyes = "focus", arm_r = { -8, 5 }, hold = { "book" }, lift = 1 }),
  } },
  -- One-shot: a page turns (the hand lifts and comes back).
  { "turn_page", {
    f(200, { sit = true, eyes = "focus", arm_r = { -6, 4 }, hold = { "book" } }),
    f(200, { sit = true, eyes = "focus", arm_r = { -5, 3 }, hold = { "book" } }),
    f(200, { sit = true, eyes = "focus", arm_r = { -8, 5 }, hold = { "book" } }),
  } },
  -- One-shot: a glance up from the page.
  { "look_up", {
    f(700, { sit = true, eyes = "up", arm_r = { -8, 5 }, hold = { "book" } }),
    f(250, { sit = true, eyes = "closed", arm_r = { -8, 5 }, hold = { "book" } }),
  } },
})
