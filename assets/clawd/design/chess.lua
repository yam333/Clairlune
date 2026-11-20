-- Chess: sitting by a low board (drawn facing right; the engine mirrors it
-- to face the table), turning a piece over while thinking, reaching out to
-- move it, then waiting for the reply.
local clawd = require("clawd")
local f = clawd.frame
local piece = { "chess_piece" }
return clawd.sheet({
  { "think", {
    f(900, { sit = true, eyes = "focus", arm_r = { 0, 2 }, hold = piece }),
    f(900, { sit = true, eyes = "focus", arm_r = { 0, 1 }, hold = piece }),
    f(160, { sit = true, eyes = "closed", arm_r = { 0, 2 }, hold = piece }),
    f(900, { sit = true, eyes = "up", arm_r = { 0, 2 }, hold = piece }),
  } },
  -- One-shot: reach over the board, set it down, sit back.
  { "move", {
    f(250, { sit = true, eyes = "right", arm_r = { 3, 1 }, hold = piece }),
    f(300, { sit = true, eyes = "right", arm_r = { 5, 2 }, hold = piece }),
    f(250, { sit = true, eyes = "happy", arm_r = { 4, 2 } }),
    f(300, { sit = true, eyes = "happy" }),
  } },
  { "wait", {
    f(900, { sit = true, eyes = "right" }),
    f(900, { sit = true, eyes = "left" }),
    f(160, { sit = true, eyes = "closed" }),
    f(900, { sit = true, eyes = "right" }),
  } },
})
