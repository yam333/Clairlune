-- A wizard practising spells beside an open spellbook. Larger canvas for the
-- hat and the raised wand; the wand tip is the `item_tip` anchor for sparkles.
local clawd = require("clawd")
local f = clawd.frame
local hat = { "wizard_hat" }
return clawd.sheet({
  { "ponder", {
    f(450, { costume = hat, eyes = "up", arm_r = { 2, -2 }, hold = { "wand", 80 } }),
    f(450, { costume = hat, eyes = "up", arm_r = { 2, -2 }, hold = { "wand", 80 }, lift = 1 }),
  } },
  { "cast", {
    f(150, { costume = hat, eyes = "happy", arm_r = { 4, -6 }, hold = { "wand", 55 } }),
    f(150, { costume = hat, eyes = "happy", arm_r = { 5, -5 }, hold = { "wand", 35 } }),
    f(150, { costume = hat, eyes = "happy", arm_r = { 4, -6 }, hold = { "wand", 55 }, lift = 1 }),
    f(150, { costume = hat, eyes = "happy", arm_r = { 3, -7 }, hold = { "wand", 75 } }),
  } },
  -- One-shot: the wand draws a circle in the air.
  { "twirl", {
    f(120, { costume = hat, eyes = "up", arm_r = { 4, -7 }, hold = { "wand", 90 } }),
    f(120, { costume = hat, eyes = "up", arm_r = { 6, -5 }, hold = { "wand", 30 } }),
    f(120, { costume = hat, eyes = "up", arm_r = { 5, -2 }, hold = { "wand", -20 } }),
    f(120, { costume = hat, eyes = "up", arm_r = { 2, -4 }, hold = { "wand", 150 } }),
    f(200, { costume = hat, eyes = "happy", arm_r = { 4, -6 }, hold = { "wand", 55 }, lift = 1 }),
  } },
}, { 40, 34, 15, 33 })
