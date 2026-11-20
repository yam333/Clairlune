-- Rowing: seated in a little boat seen from the side (the engine turns it at
-- each end of the water), one oar in the stroke. The boat is drawn around
-- the pose; its waterline is the origin row.
local clawd = require("clawd")
local f = clawd.frame
local P = clawd.prop

-- The hull, and the oar from the hand to the water at `blade` px from the
-- hand (negative: behind).
local function boat(blade)
  return function(c, a)
    local ox, oy = a.origin[1], a.origin[2]
    c.rect(ox - 12, oy - 2, 25, 1, P.wood)
    c.rect(ox - 11, oy - 1, 23, 1, P.wood_dark)
    c.rect(ox - 9, oy, 19, 1, P.wood_dark)
    c.px(ox - 13, oy - 3, P.wood)
    c.px(ox + 13, oy - 3, P.wood)
    local h = a.hand_r
    local bx = h[1] + blade
    clawd.line(c, h[1], h[2], bx, oy, P.wood)
    c.rect(bx - 1, oy, 3, 1, P.wood_dark)
  end
end

local function rowing(blade, o)
  o = o or {}
  o.facing = "side"
  o.sit = true
  o.arm_r = o.arm_r or { 2, 2 }
  o.extra = boat(blade)
  o.eyes = o.eyes or "right"
  return o
end

return clawd.sheet({
  { "row", {
    f(350, rowing(-7, { arm_r = { 3, 2 } })),
    f(350, rowing(-3, { arm_r = { 2, 2 } })),
    f(350, rowing(3, { arm_r = { 0, 2 }, lean = -1 })),
    f(350, rowing(-1, { arm_r = { 1, 3 } })),
  } },
  { "drift", {
    f(900, rowing(-6, { eyes = "happy" })),
    f(900, rowing(-6, { eyes = "up" })),
    f(160, rowing(-6, { eyes = "closed" })),
  } },
}, { 30, 20, 15, 19 })
