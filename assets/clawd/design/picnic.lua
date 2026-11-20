-- A picnic: sitting on a checked blanket (drawn under the body, in front
-- of the ground), eating an apple, sipping a warm drink (drawn facing right;
-- the engine mirrors it to face the basket).
local clawd = require("clawd")
local f = clawd.frame
local P = clawd.prop

local function blanket(c, ox, oy)
  for y = oy - 1, oy do
    for x = ox - 13, ox + 13 do
      local check = (math.floor((x - ox + 13) / 2) + (oy - y)) % 2 == 0
      c.px(x, y, check and P.red or P.white)
    end
  end
end

local function seated(o)
  o.sit = true
  o.under = blanket
  return o
end

local apple = { "apple" }
local mug = { "mug" }
return clawd.sheet({
  { "eat", {
    f(600, seated({ eyes = "happy", arm_r = { -3, 0 }, hold = apple })),
    f(500, seated({ eyes = "closed", arm_r = { -4, -1 }, hold = apple })),
    f(600, seated({ eyes = "happy", arm_r = { -3, 0 }, hold = apple })),
    f(900, seated({ eyes = "right", arm_r = { 1, 3 }, hold = apple })),
  } },
  { "sip", {
    f(900, seated({ eyes = "happy", arm_r = { -3, 1 }, hold = mug })),
    f(700, seated({ eyes = "closed", arm_r = { -4, 0 }, hold = mug })),
    f(900, seated({ eyes = "right", arm_r = { -3, 1 }, hold = mug })),
  } },
  { "look", {
    f(800, seated({ eyes = "left" })),
    f(800, seated({ eyes = "up" })),
    f(800, seated({ eyes = "right" })),
  } },
}, { 30, 20, 14, 19 })
