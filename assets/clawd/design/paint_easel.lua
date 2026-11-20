-- Painting at an easel: brush out toward the canvas (drawn facing right;
-- the engine mirrors it to face the easel), dabbing, stepping back to look.
local clawd = require("clawd")
local f = clawd.frame
local function brush(deg, o)
  o = o or {}
  o.hold = { "brush", deg }
  o.arm_r = o.arm_r or { 3, -1 }
  o.eyes = o.eyes or "right"
  return o
end
return clawd.sheet({
  { "paint", {
    f(300, brush(20, { eyes = "focus" })),
    f(300, brush(35, { eyes = "focus", arm_r = { 3, -2 } })),
    f(300, brush(10, { eyes = "focus", arm_r = { 4, 0 } })),
    f(300, brush(30, { eyes = "focus" })),
    f(160, brush(20, { eyes = "closed" })),
    f(400, brush(20)),
  } },
  -- One-shot: lean back, look, come back.
  { "step_back", {
    f(300, brush(60, { lean = -1, arm_r = { 1, 2 } })),
    f(700, brush(60, { lean = -1, arm_r = { 1, 2 }, eyes = "up" })),
    f(300, brush(20)),
  } },
  { "admire", {
    f(800, brush(70, { eyes = "happy", arm_r = { 1, 2 } })),
    f(800, brush(70, { eyes = "happy", arm_r = { 1, 2 }, lift = 1 })),
  } },
})
