-- The balloon on a journey: the same basket and envelope as balloon_ride,
-- kept to the middle of the screen while the land drifts by below.
local clawd = require("clawd")
local f = clawd.frame
local P = clawd.prop

-- The envelope: a red-and-gold striped teardrop above the head with a dark
-- rim and band, a brass skirt;
-- the basket in front of the lower body; four ropes between them.
local function balloon(c, a)
  local ox, oy = a.origin[1], a.origin[2]
  local top = a.head[2]
  local rx, ry = 11, 12
  local cy = top - 4 - ry
  for dy = -ry, ry do
    local t = dy / ry
    local w = rx * math.sqrt(math.max(0, 1 - t * t))
    if dy > 0 then w = w * (1 - 0.45 * t) end
    local half = math.floor(w + 0.5)
    for dx = -half, half do
      local col = (math.floor((dx + rx) / 4) % 2 == 0) and P.red or P.brass
      if dy == -2 then col = P.red_dark end
      -- A dark rim, so the shape holds on a light sky too.
      if dx == -half or dx == half or dy == -ry or dy == ry then col = P.red_dark end
      c.px(ox + dx, cy + dy, col)
    end
  end
  -- The skirt and the burner's mouth.
  c.rect(ox - 2, cy + ry + 1, 5, 1, P.brass)
  -- Ropes from the basket's corners straight up past the body, then in
  -- to the skirt above the head.
  local skirt = cy + ry + 1
  for _, side in ipairs({ -1, 1 }) do
    clawd.line(c, ox + side * 9, oy - 5, ox + side * 9, top - 1, P.ink)
    clawd.line(c, ox + side * 9, top - 1, ox + side * 3, skirt, P.ink)
  end
  -- The basket, in front of the legs and the lower body.
  c.rect(ox - 9, oy - 5, 19, 6, P.wood)
  for x = ox - 8, ox + 8, 2 do c.px(x, oy - 3, P.wood_dark) end
  c.rect(ox - 9, oy - 5, 19, 1, P.wood_dark)
end

local function ride(o)
  o = o or {}
  o.extra = balloon
  o.eyes = o.eyes or "right"
  return o
end

return clawd.sheet({
  { "drift", {
    f(900, ride()),
    f(900, ride({ eyes = "open" })),
    f(160, ride({ eyes = "closed" })),
    f(900, ride({ eyes = "happy" })),
  } },
  { "look", {
    f(800, ride({ eyes = "left" })),
    f(800, ride({ eyes = "up" })),
  } },
  -- One-shot: a wave over the rim.
  { "wave", {
    f(250, ride({ eyes = "happy", arm_r = "up" })),
    f(250, ride({ eyes = "happy", arm_r = "up", wiggle = true })),
    f(250, ride({ eyes = "happy", arm_r = "up" })),
    f(250, ride({ eyes = "happy", arm_r = "up", wiggle = true })),
  } },
}, { 30, 50, 14, 49 })
