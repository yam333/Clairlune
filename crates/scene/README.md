# clairlune-scene

Deterministic, seeded scene generation for [Clairlune](https://crates.io/crates/clairlune), a desktop app that makes procedural ASCII-art wallpapers.

`seed → SceneRecipe → SceneGraph`. The crate is pure: no I/O, no clocks, no OS calls. The same inputs give byte-identical output on every platform. That's why it has its own stable hashing, uses ChaCha8 only as a raw bit source with hand-written integer sampling, and uses `libm` for transcendental functions.

```rust
use clairlune_scene::{compose, build, Aspect, BuildOptions, ContentContext, Theme};

fn main() -> Result<(), clairlune_scene::BuildError> {
    let recipe = compose(7, Aspect::from_size(3840, 2160), &ContentContext::default());
    // Some activities happen only by day or only at night (a theme-locked recipe).
    let theme = if recipe.tod_for(Theme::Dark).is_some() { Theme::Dark } else { Theme::Light };
    let opts = BuildOptions { width: 3840, height: 2160, theme, buddy: true };
    let graph = build(&recipe, &opts)?;
    println!("{}", recipe.to_json()); // small, theme-independent, reproducible
    println!("{} x {} cells", graph.grid.cols, graph.grid.rows);
    Ok(())
}
```

Rendering lives in [`clairlune-render`](https://crates.io/crates/clairlune-render).

> Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character. The placeholder Clawd pixel map in `src/placeholder.rs` is fan art and is not covered by the code licence; see `NOTICE`.

License: MIT OR Apache-2.0 (code).
