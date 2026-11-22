-- Building a sandcastle: crouched beside it (drawn facing right; the
-- engine mirrors it to face the castle), digging and patting with a toy
-- spade, then sitting back to admire it.
local clawd = require("clawd")
local f = clawd.frame
local function dig(deg, o)
  o = o or {}
  o.crouch = 1
  o.hold = { "spade", deg }
  o.arm_r = o.arm_r or { 3, 3 }
  o.eyes = o.eyes or "right"
  return o
end
return clawd.sheet({
  { "dig", {
    f(350, dig(-60, { eyes = "focus" })),
    f(350, dig(-30, { eyes = "focus", arm_r = { 4, 2 } })),
    f(350, dig(-70, { eyes = "focus", arm_r = { 3, 4 } })),
    f(350, dig(-40, { eyes = "happy" })),
  } },
  { "pat", {
    f(300, dig(-10, { arm_r = { 4, 2 } })),
    f(300, dig(-10, { arm_r = { 4, 3 } })),
  } },
  { "admire", {
    f(900, { sit = true, eyes = "happy", hold = { "spade", 70 }, arm_r = { 2, 2 } }),
    f(900, { sit = true, eyes = "right", hold = { "spade", 70 }, arm_r = { 2, 2 } }),
  } },
}, { 28, 22, 11, 21 })
