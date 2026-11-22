//! Frozen scene packages: a saved scene plays exactly as it
//! did live, survives a save and load, ignores the generator (recipe, catalog,
//! built-in art) once saved, and a committed format-1 package keeps playing the
//! same frames.

use clairlune_render::clock::Clock;
use clairlune_render::{
    GlyphAtlas, IndexedPalette, PACKAGE_FORMAT, PackageError, ScenePackage, bake_static,
    render_frame_clocked,
};
use clairlune_scene::{Aspect, BuildOptions, ContentContext, SceneRecipe, Theme, build, compose};
use clairlune_tests::{blessing, golden_dir, image_hash};
use std::path::PathBuf;

const W: u32 = 480;
const H: u32 = 270;

fn recipes() -> Vec<SceneRecipe> {
    let aspect = Aspect::from_size(W, H);
    // Scenes that stay put: journeys (format 2, their background moves)
    // have their own tests in tests/journey.rs.
    let mut out: Vec<SceneRecipe> = (1u64..)
        .map(|s| compose(s, aspect, &ContentContext::default()))
        .filter(|r| !r.is_journey())
        .take(3)
        .collect();
    // Activities with anchor effects (sparkles, steam) and a weather scene.
    for (seed, activity) in [(5u64, "wizard_magic"), (6, "make_coffee"), (8, "stroll")] {
        let ctx = ContentContext {
            enabled_activities: vec![activity.to_string()],
            ..ContentContext::default()
        };
        out.push(compose(seed, aspect, &ctx));
    }
    out
}

fn times(pkg: &ScenePackage, theme: Theme) -> Vec<u64> {
    vec![0, pkg.hero_ms(theme).unwrap(), 7_777, 61_000]
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("clairlune-pkg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn a_captured_package_plays_like_the_live_scene() {
    for r in recipes() {
        let pkg = ScenePackage::capture(&r, W, H).unwrap();
        for theme in pkg.themes() {
            for buddy in [true, false] {
                let opts = BuildOptions {
                    width: W,
                    height: H,
                    theme,
                    buddy,
                };
                let graph = build(&r, &opts).unwrap();
                let atlas = GlyphAtlas::for_grid(&graph.grid);
                let palette = IndexedPalette::for_graph(&graph);
                let bg = bake_static(&graph, &atlas);
                for clock in [Clock::Live, Clock::Loop(graph.loop_ms)] {
                    for t in times(&pkg, theme) {
                        let live = render_frame_clocked(&bg, &graph, &atlas, &palette, t, clock);
                        let (frame, pal) = pkg.frame(theme, buddy, t, clock).unwrap();
                        assert!(
                            frame == live && *pal == palette,
                            "seed {} {theme:?} buddy {buddy} t {t} {clock:?}",
                            r.seed
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn save_and_load_play_the_same() {
    for (i, r) in recipes().into_iter().enumerate() {
        let pkg = ScenePackage::capture(&r, W, H).unwrap();
        let dir = temp(&format!("rt{i}"));
        pkg.save(&dir).unwrap();
        let back = ScenePackage::load(&dir).unwrap();
        assert_eq!(back.themes(), pkg.themes());
        assert_eq!(back.recipe, pkg.recipe);
        for theme in pkg.themes() {
            assert_eq!(back.hero_ms(theme), pkg.hero_ms(theme));
            for buddy in [true, false] {
                for t in times(&pkg, theme) {
                    let a = pkg.frame(theme, buddy, t, Clock::Live).unwrap();
                    let b = back.frame(theme, buddy, t, Clock::Live).unwrap();
                    assert!(
                        a.0 == b.0 && a.1 == b.1,
                        "seed {} {theme:?} {buddy} {t}",
                        r.seed
                    );
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Once saved, the generator no longer matters: change the stored recipe
/// (another generator version, another landscape and activity) and the scene
/// still plays the frames it was saved with.
#[test]
fn playback_ignores_the_generator() {
    let r = &recipes()[3];
    let pkg = ScenePackage::capture(r, W, H).unwrap();
    let dir = temp("independent");
    pkg.save(&dir).unwrap();
    let path = dir.join("package.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json["generator_version"] = 999.into();
    json["recipe"]["generator_version"] = 999.into();
    json["recipe"]["archetype"] = "cave".into();
    json["recipe"]["activity"] = "stroll".into();
    std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
    let back = ScenePackage::load(&dir).unwrap();
    for theme in pkg.themes() {
        let t = pkg.hero_ms(theme).unwrap();
        assert!(
            back.frame(theme, true, t, Clock::Live).unwrap().0
                == pkg.frame(theme, true, t, Clock::Live).unwrap().0
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Keeping one theme (what a live wallpaper plays) leaves that theme's
/// frames as they were, and asking for one the scene lacks keeps them all.
#[test]
fn keeping_one_theme_plays_it_the_same() {
    let r = &recipes()[0];
    let full = ScenePackage::capture(r, W, H).unwrap();
    assert!(full.themes().len() > 1, "{:?}", full.themes());
    let theme = full.themes()[1];
    let mut one = ScenePackage::capture(r, W, H).unwrap();
    one.keep_only(theme);
    assert_eq!(one.themes(), vec![theme]);
    for t in times(&full, theme) {
        let a = full.frame(theme, true, t, Clock::Live).unwrap().0;
        let b = one.frame(theme, true, t, Clock::Live).unwrap().0;
        assert!(a.pixels == b.pixels, "t {t}");
    }
    let mut night = ScenePackage::capture(r, W, H).unwrap();
    night.keep_only(theme);
    night.keep_only(full.themes()[0]);
    assert_eq!(night.themes(), vec![theme]);
}

#[test]
fn newer_formats_are_refused() {
    let pkg = ScenePackage::capture(&recipes()[0], W, H).unwrap();
    let dir = temp("format");
    pkg.save(&dir).unwrap();
    let path = dir.join("package.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let newer = PACKAGE_FORMAT + 1;
    std::fs::write(
        &path,
        text.replacen("\"format\":1", &format!("\"format\":{newer}"), 1),
    )
    .unwrap();
    assert!(matches!(
        ScenePackage::load(&dir),
        Err(PackageError::UnsupportedFormat(n)) if n == newer
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A saved scene damaged in ways that still parse is an error when loaded,
/// never a crash when played.
#[test]
fn a_damaged_package_is_an_error_not_a_crash() {
    use serde_json::Value;
    let ctx = ContentContext {
        enabled_activities: vec!["warm_hands".into()],
        ..ContentContext::default()
    };
    let r = compose(3, Aspect::from_size(W, H), &ctx);
    let pkg = ScenePackage::capture(&r, W, H).unwrap();
    let dir = temp("damaged");
    pkg.save(&dir).unwrap();
    let path = dir.join("package.json");
    let good: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let bump = |v: &mut Value, by: u64| *v = (v.as_u64().unwrap() + by).into();
    type Edit = Box<dyn Fn(&mut Value)>;
    let edits: Vec<(&str, Edit)> = vec![
        (
            "grid cell width",
            Box::new(move |v| bump(&mut v["themes"][0]["graph"]["grid"]["cell_w"], 2)),
        ),
        (
            "scene width",
            Box::new(move |v| bump(&mut v["themes"][0]["graph"]["width"], 64)),
        ),
        (
            "atlas cut short",
            Box::new(|v| {
                let m = v["themes"][0]["atlas"]["masks"].as_array_mut().unwrap();
                m.pop();
            }),
        ),
        (
            "prop colour",
            Box::new(|v| {
                let px = v["props"][0]["pixels"].as_array_mut().unwrap();
                let p = px.iter_mut().find(|p| !p.is_null()).unwrap();
                *p = serde_json::json!({ "r": 1, "g": 2, "b": 3 });
            }),
        ),
        (
            "ember life",
            Box::new(|v| {
                let e = &mut v["themes"][0]["graph"]["emitters"][0]["embers"][0];
                assert!(e.is_object(), "the scene has embers");
                e["life_ms"] = 0.into();
            }),
        ),
        (
            "prop size wraps",
            Box::new(|v| {
                v["props"][0]["width"] = 65_536.into();
                v["props"][0]["height"] = 65_536.into();
                v["props"][0]["pixels"] = Value::Array(Vec::new());
            }),
        ),
        (
            "frame past the sheet",
            Box::new(|v| {
                v["character"][0]["frames"][0]["y"] = u64::from(u32::MAX).into();
            }),
        ),
    ];
    for (what, edit) in &edits {
        let mut v = good.clone();
        edit(&mut v);
        std::fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
        assert!(ScenePackage::load(&dir).is_err(), "{what}: loaded");
    }
    // And the undamaged one still loads.
    std::fs::write(&path, serde_json::to_string(&good).unwrap()).unwrap();
    assert!(ScenePackage::load(&dir).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A format-1 package committed to the repository must keep playing the same
/// frames, whatever later versions change in the generator or the art. If this
/// fails, the player changed what format 1 means: keep the old behaviour for
/// format 1 and give the change a new format instead.
#[test]
fn the_committed_format_1_package_still_plays_the_same() {
    let dir = golden_dir().join("packages").join("v1-wizard");
    let hashes = golden_dir().join("packages").join("v1-wizard-frames.txt");
    if blessing() {
        let r = &recipes()[3];
        ScenePackage::capture(r, W, H).unwrap().save(&dir).unwrap();
    }
    let pkg = ScenePackage::load(&dir).expect("committed package; bless with CLAIRLUNE_BLESS=1");
    let mut lines = String::new();
    for theme in pkg.themes() {
        for buddy in [true, false] {
            for t in [pkg.hero_ms(theme).unwrap(), 12_345, 47_000] {
                let (img, pal) = pkg.frame(theme, buddy, t, Clock::Live).unwrap();
                let h = image_hash(&img, &pal.rgb_bytes());
                lines.push_str(&format!("{} {buddy} {t} {h:016x}\n", theme.as_str()));
            }
        }
    }
    if blessing() {
        std::fs::write(&hashes, &lines).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&hashes)
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(lines, expected, "format-1 playback changed");
}

/// Informational: package size at 4K (expected: about 0.3-1 MB per
/// theme). `cargo test --release --test package -- --ignored --nocapture`
#[test]
#[ignore]
fn package_size_at_4k() {
    for (i, r) in recipes().into_iter().enumerate() {
        let r = compose(
            r.seed,
            Aspect::from_size(3840, 2160),
            &ContentContext::default(),
        );
        let pkg = ScenePackage::capture(&r, 3840, 2160).unwrap();
        let dir = temp(&format!("size{i}"));
        pkg.save(&dir).unwrap();
        let mut total = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let len = e.metadata().unwrap().len();
            total += len;
            println!(
                "  {:<24} {:>8} KB",
                e.file_name().to_string_lossy(),
                len / 1024
            );
        }
        println!(
            "seed {} ({} themes): {} KB",
            r.seed,
            pkg.themes().len(),
            total / 1024
        );
        assert!(total < 2 * 1024 * 1024 * pkg.themes().len() as u64);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A saved scene exports the same GIF, byte for byte, as the live scene did.
#[test]
fn package_gifs_match_recipe_gifs() {
    use clairlune_render::{Smoothness, encode_gif, encode_package_gif};
    let r = &recipes()[4];
    let pkg = ScenePackage::capture(r, W, H).unwrap();
    for theme in pkg.themes() {
        for buddy in [true, false] {
            let opts = BuildOptions {
                width: W,
                height: H,
                theme,
                buddy,
            };
            let (mut a, mut b) = (Vec::new(), Vec::new());
            encode_gif(r, &opts, 6000, Smoothness::BatterySaver, &mut a).unwrap();
            encode_package_gif(
                &pkg,
                theme,
                buddy,
                6000,
                Smoothness::BatterySaver,
                None,
                &mut b,
            )
            .unwrap();
            assert!(a == b, "{theme:?} buddy {buddy}");
        }
    }
}
