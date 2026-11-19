-- Playing guitar by the fire: body at the strumming hand, the other hand on the neck.
local clawd = require("clawd")
local f = clawd.frame
local g = { "guitar", 165 }
return clawd.sheet({
  { "strum", {
    f(180, { eyes = "happy", arm_r = { -5, 6 }, arm_l = { 2, 4 }, hold = g }),
    f(180, { eyes = "happy", arm_r = { -5, 7 }, arm_l = { 2, 4 }, hold = g }),
    f(180, { eyes = "happy", arm_r = { -5, 6 }, arm_l = { 2, 4 }, hold = g, lift = 1 }),
    f(180, { eyes = "closed", arm_r = { -5, 7 }, arm_l = { 2, 4 }, hold = g, lift = 1 }),
  } },
  { "pause", {
    f(600, { eyes = "open", arm_r = { -5, 6 }, arm_l = { 2, 4 }, hold = g }),
    f(600, { eyes = "right", arm_r = { -5, 6 }, arm_l = { 2, 4 }, hold = g }),
  } },
})
