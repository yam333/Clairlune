# Takedown procedure

If Anthropic asks the project to stop using Clawd, comply quickly. The code is arranged so that this is a small change.

## Steps

1. Remove all Clawd artwork:
   - `assets/clawd/` (from milestone M0 onward)
   - the `CLAWD_PLACEHOLDER` pixel map in `crates/scene/src/placeholder.rs` (M1 placeholder)
2. Set the Buddy default to off (Settings → Buddy → "Show Clawd in wallpapers"). Until the sprite module exists, the CLI equivalent is `--no-buddy`.
3. Make scenes render the activity's unattended variant only. Buddy is a render flag, not part of the recipe, so archived recipes stay valid.
4. Remove the character from screenshots, the README and any store listing.
5. Publish a new release, and yank crates.io versions that contain the artwork (`cargo yank --version <v> <crate>`). Yanking does not delete a version, so say so in the reply.
6. Reply to Anthropic with what was removed and when.

## Later option

Replace the character with an original one. All character code is meant to live behind the `Character` trait in `sprites::clawd` (milestone M2), so a replacement only touches that module and `assets/clawd/`.
