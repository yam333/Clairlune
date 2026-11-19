-- Template: canvas, guides, anchors layer and the neutral pose. Copy it to start
-- a new activity (it is not exported).
local clawd = require("clawd")
return clawd.sheet({
  { "neutral", { clawd.frame(1000, {}) } },
})
