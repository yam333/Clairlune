-- Tennis against a practice wall (drawn facing right; the engine mirrors it
-- to face the wall). One forehand per 1.2 s: the ball (the engine's
-- projectile) leaves the racket on the first frame and is back on it as the
-- loop comes round.
local clawd = require("clawd")
local f = clawd.frame
local function swing(deg, arm, o)
  o = o or {}
  o.hold = { "racket", deg }
  o.arm_r = arm
  o.eyes = o.eyes or "right"
  return o
end
return clawd.sheet({
  { "rally", {
    f(300, swing(20, { 3, 0 }, { eyes = "focus" })),
    f(300, swing(60, { 1, -2 })),
    f(300, swing(150, { -2, 1 })),
    f(300, swing(110, { 0, 2 }, { eyes = "focus" })),
  } },
  { "rest", {
    f(900, swing(270, { 1, 3 }, { eyes = "happy" })),
    f(900, swing(270, { 1, 3 }, { eyes = "happy", lift = 1 })),
    f(160, swing(270, { 1, 3 }, { eyes = "closed" })),
  } },
}, { 30, 20, 12, 19 })
