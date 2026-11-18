# clairlune-render

Rendering for [Clairlune](https://crates.io/crates/clairlune) ASCII-art wallpapers: glyph atlas, palette-index compositor, frame compositing and indexed PNG export.

Every pixel is a palette index. Colours are looked up, never blended at render time, so a scene has at most 255 colours and stills, GIF frames and the live renderer can match exactly. The glyph rasterizer is self-contained (outlines from `ttf-parser`, 8×8 supersampled coverage using only IEEE-exact arithmetic), so output is byte-identical across operating systems and CPUs.

```rust,no_run
use clairlune_render::{render_still, save_png};
use clairlune_scene::{compose, Aspect, BuildOptions, ContentContext, Theme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recipe = compose(7, Aspect::from_size(1920, 1080), &ContentContext::default());
    let opts = BuildOptions { width: 1920, height: 1080, theme: Theme::Light, buddy: true };
    let still = render_still(&recipe, &opts, None)?; // None = the hero frame
    save_png("wallpaper.png".as_ref(), &still.image, &still.palette)?;
    Ok(())
}
```

> Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character.

License: code under MIT OR Apache-2.0. The bundled JetBrains Mono font (`assets/fonts/`) is under the SIL Open Font License 1.1.
