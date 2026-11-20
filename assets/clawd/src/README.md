# Clawd sprite sources

Aseprite source files for the project's own Clawd drawings. Read `docs/clawd-style.md` first.

| File | Contents |
|---|---|
| `_template.aseprite` | Canvas, palette, guides, standard slices. Copy it to start an activity. |
| `_base.aseprite` | Shared animations: `idle`, `walk`, `celebrate`, `love`, `alert`, `dizzy`. |
| `<activity>.aseprite` | One file per activity id; one tag per state. |

After saving, export with `python scripts/export_sprites.py <name>` and commit the source and the export together. To see each save in a real scene, keep `cargo run -p clairlune-cli -- preview-activity <name> --export` open: it re-exports on save and reloads.

These drawings are fan art of Anthropic's character and are not under the code licence; see `NOTICE`.
