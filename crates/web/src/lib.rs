//! Clairlune's engine for the website: the scene and render crates built for
//! WebAssembly and driven from JavaScript through a handful of plain
//! functions. There is no bindings generator. Numbers go in and out; text
//! goes through two buffers (JSON in, JSON or PNG bytes out); pixels are read
//! straight out of the module's memory, and only the rectangles a frame
//! changed are reported, the way the desktop presenter works.
//!
//! The scenes are the app's own: the same seed and choices draw the same
//! wallpaper here as in Clairlune and `clairlune-cli`.

use clairlune_render::{
    BuiltinArt, Clock, GlyphAtlas, IncrementalFrame, IndexedImage, IndexedPalette, JourneyBake,
    bake_static, encode_png, hero_time,
};
use clairlune_scene::activity::builtin_catalog;
use clairlune_scene::archetype::{ARCHETYPE_INFO, WEATHERS};
use clairlune_scene::glyphs::{GLYPHS, LETTERS, letters};
use clairlune_scene::palette::{PALETTES, Rgb, families_in};
use clairlune_scene::recipe::{CONTRASTS, LOOKS};
use clairlune_scene::{
    Aspect, BuildError, BuildOptions, ContentContext, SceneGraph, SceneRecipe, Theme, ZOOMS, build,
    compose,
};
use clairlune_sprites::{Character, builtin_character};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;

/// The names Settings shows for landscapes, activities, weather, palettes and
/// letter sets: the app's own table, shared so the two never disagree.
#[allow(dead_code)]
#[path = "../../app/src/ui/view/catalog.rs"]
mod catalog;

/// The largest scene the page may ask for (a 6K screen at 1x).
const MAX_PIXELS: u64 = 6144 * 3456;

/// What a scene is made from, as the page sends it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Options {
    /// A whole number, as a string (JavaScript numbers lose digits past 2^53)
    /// or a number.
    seed: Value,
    width: u32,
    height: u32,
    /// `light` (day) or `dark` (night).
    theme: String,
    /// Clawd in the scene.
    buddy: Option<bool>,
    landscape: Option<String>,
    activity: Option<String>,
    weather: Option<String>,
    palette: Option<String>,
    letters: Option<String>,
    look: Option<String>,
    contrast: Option<String>,
    zoom: Option<String>,
    /// When the scene's activity only happens at the other time of day, show
    /// it then instead of failing (as `clairlune-cli` does).
    fallback: bool,
}

/// A scene made and ready to play.
struct Scene {
    recipe: SceneRecipe,
    graph: SceneGraph,
    atlas: GlyphAtlas,
    palette: IndexedPalette,
    /// Palette index to RGBA.
    lut: Vec<[u8; 4]>,
    background: IndexedImage,
    journey: Option<JourneyBake>,
    art: BuiltinArt,
    frame: IncrementalFrame,
    rgba: Vec<u8>,
    /// x, y, w, h of each rectangle the last frame changed.
    rects: Vec<u32>,
    /// Nothing drawn into `rgba` yet: the next frame reports all of it.
    fresh: bool,
}

#[derive(Default)]
struct Engine {
    input: Vec<u8>,
    output: Vec<u8>,
    scenes: Vec<Option<Scene>>,
}

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::default());
}

fn with<R>(f: impl FnOnce(&mut Engine) -> R) -> R {
    ENGINE.with(|e| f(&mut e.borrow_mut()))
}

fn scene_mut(e: &mut Engine, handle: u32) -> Option<&mut Scene> {
    e.scenes
        .get_mut((handle as usize).checked_sub(1)?)
        .and_then(Option::as_mut)
}

// ---------------------------------------------------------------------------
// The functions JavaScript calls.

/// Makes the input buffer `len` bytes long and returns where to write.
#[unsafe(no_mangle)]
pub extern "C" fn cl_input(len: u32) -> *mut u8 {
    with(|e| {
        e.input.clear();
        e.input.resize(len as usize, 0);
        e.input.as_mut_ptr()
    })
}

/// Where the last answer is.
#[unsafe(no_mangle)]
pub extern "C" fn cl_output() -> *const u8 {
    with(|e| e.output.as_ptr())
}

/// How long the last answer is.
#[unsafe(no_mangle)]
pub extern "C" fn cl_output_len() -> u32 {
    with(|e| e.output.len() as u32)
}

/// Writes what scenes can be made of (JSON) and returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn cl_catalog() -> u32 {
    let text = catalog_json().to_string();
    with(|e| {
        e.output = text.into_bytes();
        e.output.len() as u32
    })
}

/// Makes a scene from the options in the input buffer. Returns its handle
/// (with what it is, as JSON, in the output), or 0 with the reason in the
/// output.
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_new() -> u32 {
    let input = with(|e| std::mem::take(&mut e.input));
    let made = serde_json::from_slice::<Options>(&input)
        .map_err(|e| format!("options: {e}"))
        .and_then(|o| make_scene(&o));
    with(|e| match made {
        Ok(scene) => {
            e.output = info(&scene).to_string().into_bytes();
            let slot = e.scenes.iter().position(Option::is_none);
            let i = match slot {
                Some(i) => {
                    e.scenes[i] = Some(scene);
                    i
                }
                None => {
                    e.scenes.push(Some(scene));
                    e.scenes.len() - 1
                }
            };
            i as u32 + 1
        }
        Err(msg) => {
            e.output = msg.into_bytes();
            0
        }
    })
}

/// Moves a scene to `t_ms` and returns how many rectangles changed (their
/// x, y, w, h at [`cl_scene_rects`], the pixels at [`cl_scene_rgba`]).
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_frame(handle: u32, t_ms: f64) -> u32 {
    with(|e| scene_mut(e, handle).map_or(0, |s| s.advance(t_ms.max(0.0) as u64)))
}

/// The scene's pixels, RGBA, row by row.
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_rgba(handle: u32) -> *const u8 {
    with(|e| scene_mut(e, handle).map_or(std::ptr::null(), |s| s.rgba.as_ptr()))
}

/// The rectangles the last frame changed: x, y, w, h each.
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_rects(handle: u32) -> *const u32 {
    with(|e| scene_mut(e, handle).map_or(std::ptr::null(), |s| s.rects.as_ptr()))
}

/// The scene's best moment for a still, in milliseconds.
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_hero_ms(handle: u32) -> f64 {
    with(|e| scene_mut(e, handle).map_or(0.0, |s| hero_time(&s.graph) as f64))
}

/// Encodes the scene's current frame as a PNG into the output and returns
/// its length (0 when it can't).
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_png(handle: u32) -> u32 {
    with(|e| {
        let Some(s) = scene_mut(e, handle) else {
            return 0;
        };
        if s.fresh {
            s.advance(hero_time(&s.graph));
        }
        let mut png = Vec::new();
        if encode_png(s.frame.image(), &s.palette, &mut png).is_err() {
            return 0;
        }
        e.output = png;
        e.output.len() as u32
    })
}

/// Lets a scene go.
#[unsafe(no_mangle)]
pub extern "C" fn cl_scene_free(handle: u32) {
    with(|e| {
        if let Some(slot) = (handle as usize)
            .checked_sub(1)
            .and_then(|i| e.scenes.get_mut(i))
        {
            *slot = None;
        }
    });
}

/// Clawd's frames for one of his own states (`idle`, `walk`, `celebrate`,
/// `alert`; the state's name in the input). The output holds, in
/// little-endian u32s: the frame count, then for each frame its width,
/// height, duration in ms and origin x and y, followed by its RGBA pixels.
/// Returns the output's length (0: no such state).
#[unsafe(no_mangle)]
pub extern "C" fn cl_clawd() -> u32 {
    let state = with(|e| String::from_utf8_lossy(&std::mem::take(&mut e.input)).into_owned());
    let bytes = clawd_frames(state.trim());
    with(|e| {
        e.output = bytes;
        e.output.len() as u32
    })
}

// ---------------------------------------------------------------------------

impl Scene {
    fn advance(&mut self, t_ms: u64) -> u32 {
        let changed = match &self.journey {
            Some(j) => self.frame.advance_journey(
                j,
                &self.background,
                &self.graph,
                &self.atlas,
                &self.palette,
                t_ms,
                Clock::Smooth,
                &self.art,
            ),
            None => self.frame.advance(
                &self.background,
                &self.graph,
                &self.atlas,
                &self.palette,
                t_ms,
                Clock::Smooth,
                &self.art,
            ),
        };
        let img = self.frame.image();
        let (w, h) = (img.width, img.height);
        self.rects.clear();
        if std::mem::take(&mut self.fresh) {
            self.rects.extend([0, 0, w, h]);
        } else {
            for r in &changed {
                self.rects.extend([r.x, r.y, r.w, r.h]);
            }
        }
        for &[x, y, rw, rh] in self.rects.as_chunks::<4>().0 {
            for row in y..(y + rh).min(h) {
                let start = (row * w + x) as usize;
                let end = (row * w + (x + rw).min(w)) as usize;
                for (i, &p) in img.pixels[start..end].iter().enumerate() {
                    let o = (start + i) * 4;
                    self.rgba[o..o + 4].copy_from_slice(&self.lut[p as usize]);
                }
            }
        }
        (self.rects.len() / 4) as u32
    }
}

fn seed_of(v: &Value) -> Result<u64, String> {
    match v {
        Value::Null => Ok(1),
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| "a seed is a whole number of up to 20 digits".to_string()),
        Value::String(s) => s
            .trim()
            .parse::<u64>()
            .map_err(|_| "a seed is a whole number of up to 20 digits".to_string()),
        _ => Err("a seed is a whole number of up to 20 digits".into()),
    }
}

/// Names a choice the page made, or says which ones exist (the CLI's words).
fn pick(
    what: &str,
    value: &Option<String>,
    known: &[&str],
    plural: &str,
) -> Result<Option<String>, String> {
    match value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) if known.contains(&v) => Ok(Some(v.to_string())),
        Some(v) => Err(format!(
            "unknown {what} `{v}`; {plural}: {}",
            known.join(", ")
        )),
    }
}

fn landscape_ids() -> Vec<&'static str> {
    ARCHETYPE_INFO
        .iter()
        .filter(|a| a.implemented)
        .map(|a| a.id)
        .collect()
}

fn make_scene(o: &Options) -> Result<Scene, String> {
    let seed = seed_of(&o.seed)?;
    let (width, height) = (o.width.max(1), o.height.max(1));
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!("{width}x{height} is bigger than a 6K screen"));
    }
    let theme = match o.theme.as_str() {
        "" | "dark" => Theme::Dark,
        "light" => Theme::Light,
        other => return Err(format!("unknown theme `{other}`; themes: light, dark")),
    };
    // Composed as clairlune-cli composes, so a seed and the same flags draw
    // the same scene in the terminal as here.
    let mut ctx = ContentContext::default();
    let landscapes = landscape_ids();
    if let Some(id) = pick("landscape", &o.landscape, &landscapes, "available")? {
        ctx.enabled_archetypes = vec![id];
    }
    let activities: Vec<&str> = builtin_catalog()
        .activities
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    if let Some(id) = pick("activity", &o.activity, &activities, "available")? {
        ctx.enabled_activities = vec![id];
    }
    if let Some(id) = pick("weather", &o.weather, &WEATHERS, "available")? {
        ctx.enabled_weather = vec![id];
    }
    if let Some(id) = pick("palette", &o.palette, &PALETTES, "palettes")? {
        ctx.enabled_palettes = vec![id];
    }
    if let Some(id) = pick("letter set", &o.letters, &LETTERS, "letter sets")? {
        ctx.enabled_letters = vec![id];
    }
    if let Some(id) = pick("look", &o.look, &LOOKS, "looks")? {
        ctx.enabled_looks = vec![id];
    }
    if let Some(id) = pick("zoom", &o.zoom, &ZOOMS, "zooms")? {
        ctx.enabled_zooms = vec![id];
    }
    let mut contrasts: Vec<&str> = CONTRASTS.to_vec();
    contrasts.push("vary");
    if let Some(c) = pick("contrast", &o.contrast, &contrasts, "contrasts")? {
        ctx.contrast = c;
    }
    let recipe = compose(seed, Aspect::from_size(width, height), &ctx);
    let opts = |theme| BuildOptions {
        width,
        height,
        theme,
        buddy: o.buddy.unwrap_or(true),
    };
    let graph = match build(&recipe, &opts(theme)) {
        Err(BuildError::ThemeLocked(_)) if o.fallback => {
            let other = match theme {
                Theme::Light => Theme::Dark,
                Theme::Dark => Theme::Light,
            };
            build(&recipe, &opts(other))
        }
        built => built,
    }
    .map_err(|e| match e {
        BuildError::ThemeLocked(t) => format!(
            "seed {seed} ({}) has no {} variant: the activity only happens at {}. Use the other theme or another seed.",
            recipe.activity.as_deref().unwrap_or("no activity"),
            t.as_str(),
            if t == Theme::Light { "night" } else { "daytime" },
        ),
        other => other.to_string(),
    })?;
    let atlas = GlyphAtlas::for_graph(&graph);
    let palette = IndexedPalette::for_graph(&graph);
    let mut lut = vec![[0, 0, 0, 255]; 256];
    for (slot, c) in lut.iter_mut().zip(palette.colors()) {
        *slot = [c.r, c.g, c.b, 255];
    }
    let background = bake_static(&graph, &atlas);
    let journey = JourneyBake::new(&graph, &atlas);
    let art = BuiltinArt::for_graph(&graph);
    let frame = IncrementalFrame::new(&background);
    let rgba = vec![0; width as usize * height as usize * 4];
    Ok(Scene {
        recipe,
        graph,
        atlas,
        palette,
        lut,
        background,
        journey,
        art,
        frame,
        rgba,
        rects: Vec::new(),
        fresh: true,
    })
}

fn name_of(table: &[catalog::Item], id: &str) -> String {
    table
        .iter()
        .find(|(i, _, _)| *i == id)
        .map_or_else(|| id.replace('_', " "), |(_, n, _)| (*n).to_string())
}

/// What a scene is, for the page: names, ids and its size.
fn info(s: &Scene) -> Value {
    let r = &s.recipe;
    let landscape = name_of(&catalog::LANDSCAPES, &r.archetype);
    let activity = r
        .activity
        .as_deref()
        .filter(|_| s.graph.buddy)
        .map(|a| name_of(&catalog::ACTIVITIES, a));
    let set = clairlune_scene::palette::family(match s.graph.theme {
        Theme::Light => &r.palette.light,
        Theme::Dark => &r.palette.dark,
    })
    .map_or("classic", |f| f.set);
    json!({
        "seed": r.seed.to_string(),
        "title": match &activity {
            Some(a) => format!("{a} · {landscape}"),
            None => landscape.clone(),
        },
        "landscape": r.archetype,
        "landscapeName": landscape,
        "activity": r.activity.as_deref().filter(|_| s.graph.buddy),
        "activityName": activity,
        "weather": r.weather,
        "weatherName": name_of(&catalog::WEATHER, &r.weather),
        "palette": set,
        "paletteName": name_of(&catalog::PALETTES, set),
        "letters": r.letters,
        "look": r.look,
        "zoom": r.zoom,
        "theme": s.graph.theme.as_str(),
        "buddy": s.graph.buddy,
        "journey": s.journey.is_some(),
        "width": s.frame.image().width,
        "height": s.frame.image().height,
        "heroMs": hero_time(&s.graph),
    })
}

fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// Everything the page offers to choose from.
fn catalog_json() -> Value {
    let landscapes: Vec<Value> = landscape_ids()
        .into_iter()
        .map(|id| json!({ "id": id, "name": name_of(&catalog::LANDSCAPES, id) }))
        .collect();
    let activities: Vec<Value> = builtin_catalog()
        .activities
        .iter()
        .map(|a| json!({ "id": a.id, "name": name_of(&catalog::ACTIVITIES, &a.id) }))
        .collect();
    let weather: Vec<Value> = WEATHERS
        .iter()
        .map(|id| json!({ "id": id, "name": name_of(&catalog::WEATHER, id) }))
        .collect();
    let palettes: Vec<Value> = PALETTES
        .iter()
        .map(|id| {
            let theme = |t: Theme| {
                families_in(id, t).next().map(|f| {
                    json!({
                        "bg": hex(f.bg_center),
                        "edge": hex(f.bg_edge),
                        "ink": hex(f.ink),
                        "accent": hex(f.accent),
                        "ember": hex(f.ember),
                    })
                })
            };
            json!({
                "id": id,
                "name": name_of(&catalog::PALETTES, id),
                "light": theme(Theme::Light),
                "dark": theme(Theme::Dark),
            })
        })
        .collect();
    let sets: Vec<Value> = LETTERS
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "name": name_of(&catalog::LETTERS, id),
                "chars": letters(id).iter().collect::<String>(),
            })
        })
        .collect();
    json!({
        "landscapes": landscapes,
        "activities": activities,
        "weather": weather,
        "palettes": palettes,
        "letters": sets,
        "glyphs": GLYPHS.iter().collect::<String>(),
        "looks": LOOKS,
        "zooms": ZOOMS,
    })
}

fn clawd_frames(state: &str) -> Vec<u8> {
    let Some(anim) = builtin_character().animation("_base", state) else {
        return Vec::new();
    };
    if anim.name != state {
        // Fell back to the placeholder: not one of his own states.
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut put = |v: u32| out.extend_from_slice(&v.to_le_bytes());
    put(anim.frames.len() as u32);
    let mut pixels = Vec::new();
    for f in &anim.frames {
        let (ox, oy) = f.origin();
        for v in [
            f.width,
            f.height,
            f.duration_ms,
            ox.max(0) as u32,
            oy.max(0) as u32,
        ] {
            put(v);
        }
        for p in &f.pixels {
            pixels.extend_from_slice(&match p {
                Some(c) => [c.r, c.g, c.b, 255],
                None => [0, 0, 0, 0],
            });
        }
    }
    out.extend(pixels);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_scene(opts: &str) -> Result<(u32, Value), String> {
        let bytes = opts.as_bytes();
        let ptr = cl_input(bytes.len() as u32);
        with(|e| {
            assert_eq!(ptr, e.input.as_mut_ptr());
            e.input.copy_from_slice(bytes);
        });
        let h = cl_scene_new();
        let out = with(|e| String::from_utf8(e.output.clone()).unwrap());
        if h == 0 {
            Err(out)
        } else {
            Ok((h, serde_json::from_str(&out).unwrap()))
        }
    }

    #[test]
    fn a_scene_plays_and_redraws_only_what_moves() {
        let (h, info) = new_scene(
            r#"{"seed":"7","width":320,"height":180,"theme":"dark","landscape":"mountains","fallback":true}"#,
        )
        .unwrap();
        assert_eq!(info["seed"], "7");
        assert_eq!(info["landscape"], "mountains");
        assert_eq!(info["width"], 320);
        // The first frame is all of it.
        assert_eq!(cl_scene_frame(h, 0.0), 1);
        let first = with(|e| scene_mut(e, h).unwrap().rects.clone());
        assert_eq!(first, vec![0, 0, 320, 180]);
        // Later frames report pieces, and the pixels match a whole render.
        let n = cl_scene_frame(h, 1000.0);
        let (rgba, expect) = with(|e| {
            let s = scene_mut(e, h).unwrap();
            let full = clairlune_render::to_rgba(s.frame.image(), &s.palette);
            (s.rgba.clone(), full)
        });
        assert!(n >= 1);
        assert_eq!(rgba, expect);
        assert!(cl_scene_png(h) > 100);
        assert_eq!(&with(|e| e.output[1..4].to_vec()), b"PNG");
        cl_scene_free(h);
        assert_eq!(cl_scene_frame(h, 0.0), 0);
    }

    #[test]
    fn the_same_seed_draws_the_same_scene_as_the_cli() {
        let (h, _) = new_scene(r#"{"seed":42,"width":256,"height":144,"theme":"light"}"#).unwrap();
        let hero = cl_scene_hero_ms(h);
        cl_scene_frame(h, hero);
        let ours = with(|e| scene_mut(e, h).unwrap().frame.image().pixels.clone());
        // clairlune-cli composes with the default context, theme unknown.
        let recipe = compose(42, Aspect::from_size(256, 144), &ContentContext::default());
        let opts = BuildOptions {
            width: 256,
            height: 144,
            theme: Theme::Light,
            buddy: true,
        };
        let still = clairlune_render::render_still(&recipe, &opts, Some(hero as u64)).unwrap();
        assert_eq!(ours, still.image.pixels);
        cl_scene_free(h);
    }

    #[test]
    fn a_scene_of_the_other_time_of_day_is_shown_then_or_refused() {
        // Find a seed whose scene only happens at night, the CLI's way.
        let seed = (1..500u64)
            .find(|&seed| {
                let r = compose(
                    seed,
                    Aspect::from_size(320, 180),
                    &ContentContext::default(),
                );
                let o = BuildOptions {
                    width: 320,
                    height: 180,
                    theme: Theme::Light,
                    buddy: true,
                };
                matches!(build(&r, &o), Err(BuildError::ThemeLocked(Theme::Light)))
            })
            .expect("some scene only happens at night");
        let err = new_scene(&format!(
            r#"{{"seed":{seed},"width":320,"height":180,"theme":"light"}}"#
        ))
        .unwrap_err();
        assert!(err.contains("has no light variant"), "{err}");
        let (h, info) = new_scene(&format!(
            r#"{{"seed":{seed},"width":320,"height":180,"theme":"light","fallback":true}}"#
        ))
        .unwrap();
        assert_eq!(info["theme"], "dark");
        cl_scene_free(h);
    }

    #[test]
    fn unknown_choices_say_what_exists() {
        let err =
            new_scene(r#"{"seed":1,"width":64,"height":36,"landscape":"atlantis"}"#).unwrap_err();
        assert!(
            err.starts_with("unknown landscape `atlantis`; available: "),
            "{err}"
        );
        assert!(err.contains("fjord"));
        let err = new_scene(r#"{"seed":"x","width":64,"height":36}"#).unwrap_err();
        assert!(err.contains("whole number"), "{err}");
    }

    #[test]
    fn the_catalog_names_everything() {
        let n = cl_catalog();
        let text = with(|e| e.output.clone());
        assert_eq!(n as usize, text.len());
        let c: Value = serde_json::from_slice(&text).unwrap();
        assert_eq!(
            c["landscapes"].as_array().unwrap().len(),
            landscape_ids().len()
        );
        assert_eq!(c["palettes"].as_array().unwrap().len(), PALETTES.len());
        assert!(
            c["palettes"][1]["dark"]["bg"]
                .as_str()
                .unwrap()
                .starts_with('#')
        );
        let sets = c["letters"].as_array().unwrap();
        for s in sets {
            assert_eq!(
                s["chars"].as_str().unwrap().chars().count(),
                GLYPHS.len(),
                "{s}"
            );
        }
        // Every landscape has a name of its own, not an id.
        for l in c["landscapes"].as_array().unwrap() {
            assert_ne!(l["name"], l["id"], "{l}");
        }
    }

    #[test]
    fn clawd_walks() {
        let input = b"walk";
        let ptr = cl_input(input.len() as u32);
        with(|e| {
            assert_eq!(ptr, e.input.as_mut_ptr());
            e.input.copy_from_slice(input);
        });
        let n = cl_clawd();
        let out = with(|e| e.output.clone());
        assert_eq!(n as usize, out.len());
        let frames = u32::from_le_bytes(out[0..4].try_into().unwrap());
        assert!(frames >= 2, "{frames}");
        let w = u32::from_le_bytes(out[4..8].try_into().unwrap());
        let h = u32::from_le_bytes(out[8..12].try_into().unwrap());
        assert!(w > 4 && h > 4);
        assert!(clawd_frames("no_such_state").is_empty());
    }
}
