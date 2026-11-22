-- Pose catalogue: one key pose per planned activity, to check that the rig can
-- express them (docs/dev/activity-system.md). Not exported (name starts with
-- `_template`). Extras sketch where scene effects attach; in the engine those
-- are effects and set pieces, not part of the character sheet.

local clawd = require("clawd")
local f = clawd.frame
local P = clawd.prop
local line = clawd.line

local function spray(c, a) -- snow spray behind the skis
  local x, y = a.origin[1] - 11, a.origin[2] - 1
  for _, d in ipairs({ { 0, 0 }, { -2, -2 }, { -1, -4 }, { -4, -1 }, { -3, -5 } }) do
    c.px(x + d[1], y + d[2], P.white)
  end
end

local function steam(c, a)
  local x, y = a.item_tip[1], a.item_tip[2]
  for i, d in ipairs({ { 0, -1 }, { 1, -3 }, { 0, -5 }, { 1, -7 } }) do
    c.px(x + d[1], y + d[2], i % 2 == 0 and P.white or { 0xC8, 0xC4, 0xBA })
  end
end

local function chessboard(c, a)
  local x0, y0 = a.origin[1] - 7, a.origin[2] - 4
  for i = 0, 14 do
    for j = 0, 1 do
      c.px(x0 + i, y0 + j, ((i + j) % 2 == 0) and P.white or P.ink)
    end
  end
  c.rect(x0 - 2, y0 + 2, 19, 1, P.wood) -- table top
  c.rect(x0 - 1, y0 + 3, 1, 2, P.wood_dark)
  c.rect(x0 + 15, y0 + 3, 1, 2, P.wood_dark)
end

local function ball(c, a)
  local x, y = a.item_tip[1] + 6, a.item_tip[2] - 3
  c.rect(x, y, 2, 2, P.ball)
end

local function sparkles(c, a)
  local x, y = a.item_tip[1], a.item_tip[2]
  for _, d in ipairs({ { 2, -2 }, { -2, -3 }, { 3, 1 }, { 0, -5 }, { 5, -4 } }) do
    c.px(x + d[1], y + d[2], P.spark)
  end
  c.px(x + 1, y - 1, P.white)
end

return clawd.sheet({
  { "ski", { f(200, {
    facing = "side", legs = "ski", crouch = 1, lean = 2, eyes = "focus",
    costume = { "beanie", "goggles" },
    arm_r = { 3, 2 }, hold = { "ski_pole", -125 },
    extra = spray,
  }) } },
  { "coffee", { f(200, {
    eyes = "happy",
    arm_r = { -3, 0 }, hold = { "mug" },
    extra = steam,
  }) } },
  { "hike", { f(200, {
    facing = "side", legs = "walk_a", lean = 1,
    costume = { "backpack", "beanie" },
    arm_r = { 3, 2 }, hold = { "walking_stick", -75 },
  }) } },
  { "chess", { f(200, {
    sit = true, eyes = "focus",
    arm_r = { -8, 3 }, hold = { "chess_piece" },
    extra = chessboard,
  }) } },
  { "tennis", { f(200, {
    facing = "side", legs = "walk_b", lean = 1, eyes = "focus",
    arm_r = { 5, -3 }, hold = { "racket", 35 },
    extra = ball,
  }) } },
  { "wizard", { f(200, {
    eyes = "happy", costume = { "wizard_hat" },
    arm_r = { 4, -6 }, hold = { "wand", 55 },
    extra = sparkles,
  }) } },
}, { 44, 34, 21, 31 })
