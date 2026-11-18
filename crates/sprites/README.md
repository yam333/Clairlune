# clairlune-sprites

Sprite sheets for [Clairlune](https://crates.io/crates/clairlune): loads Aseprite `json-array` exports, reads per-frame anchors from slices, validates the pixel-art rules (no partial alpha, at most 16 character colours, an `origin` anchor on every frame), and exposes the `Character` trait the renderer draws through.

> Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character. Sheets in `assets/clawd/` are fan art and are not covered by the code licence; see `NOTICE`.

License: MIT OR Apache-2.0 (code).
