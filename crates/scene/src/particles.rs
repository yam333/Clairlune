//! Seeded, periodic particles. Each particle's state is a pure function of
//! `t mod life`, and every life divides the loop length, so animations loop
//! seamlessly (needed for GIF export, free for live mode).

use crate::glyphs::EMBERS;
use crate::math::{TAU, pow, sin};
use crate::rng::Rng;

/// Loop length shared by all periodic elements in M1.
pub const LOOP_MS: u64 = 6000;
/// Particle lifetimes. Each divides [`LOOP_MS`].
const LIVES_MS: [u32; 4] = [1000, 1500, 2000, 3000];

/// How fast particles move. Pace 0 is the original motion, kept so scenes
/// saved with it play exactly as they were saved. Pace 1 (generator 4 on)
/// is calm: embers drift up slowly, snow and dust float, effects linger, so
/// a smooth frame rate shows slow motion rather than a frantic one. Every
/// life still divides [`LOOP_MS`], so GIFs loop seamlessly.
pub const CALM_PACE: u8 = 1;
/// Calm ember lives.
const CALM_LIVES_MS: [u32; 3] = [2000, 3000, 6000];
/// Hero-time search: candidates every 500 ms in the first 10 s.
const HERO_STEP_MS: u64 = 500;
const HERO_CANDIDATES: u64 = 20;
/// Clustering bucket size in units of image height.
const HERO_BUCKET: f64 = 0.03;

/// One ember, in units of image height, relative to the emitter origin.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Ember {
    life_ms: u32,
    phase_ms: u32,
    x0: f64,
    drift: f64,
    rise: f64,
    wobble: f64,
    wobble_cycles: f64,
    wobble_phase: f64,
    max_level: f64,
    glyph_bias: u32,
}

/// A particle sample in pixels with an ink level `1..=6`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub glyph: u8,
    pub level: u8,
    /// Drawn in ink colour (snow, rain, dust) instead of ember colour.
    pub ink: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitterKind {
    /// Upward-drifting plume that widens with height.
    Embers,
    /// Screen-wide weather particles.
    Weather(WeatherKind),
}

/// A particle emitter placed in the image.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Emitter {
    pub kind: EmitterKind,
    /// Origin in pixels.
    pub origin_x: f64,
    pub origin_y: f64,
    /// Pixels per normalised unit (the image height).
    pub unit: f64,
    /// Multiplies every particle's level (dimmer in daylight).
    pub intensity: f64,
    pub embers: Vec<Ember>,
    /// Weather particles (empty for embers).
    pub drops: Vec<Drop>,
    /// Image size in pixels, for screen-space weather.
    pub size: (f64, f64),
    /// How fast the weather moves (see [`CALM_PACE`]); 0 in older scenes.
    #[serde(default)]
    pub pace: u8,
}

/// The ember set for a seed at a pace. Independent of size and theme, so the
/// hero time is too.
pub fn ember_set(seed: u64, pace: u8) -> Vec<Ember> {
    if pace >= CALM_PACE {
        return calm_ember_set(seed);
    }
    let mut rng = Rng::stream(seed, "particles");
    let count = rng.range_u32(44, 72);
    (0..count)
        .map(|_| {
            let life_ms = LIVES_MS[rng.weighted_index(&[1, 2, 3, 2])];
            Ember {
                life_ms,
                phase_ms: rng.below(life_ms),
                x0: rng.range_f64(-0.010, 0.010),
                drift: rng.range_f64(-0.075, 0.075),
                rise: rng.range_f64(0.16, 0.38),
                wobble: rng.range_f64(0.002, 0.012),
                wobble_cycles: f64::from(rng.range_u32(1, 3)),
                wobble_phase: rng.unit(),
                max_level: f64::from(rng.range_u32(4, 6)),
                glyph_bias: rng.below(2),
            }
        })
        .collect()
}

/// Calm embers: longer lives, a short rise and little drift, so they float up
/// from the fire instead of shooting off.
fn calm_ember_set(seed: u64) -> Vec<Ember> {
    let mut rng = Rng::stream(seed, "particles_calm");
    let count = rng.range_u32(36, 56);
    (0..count)
        .map(|_| {
            let life_ms = CALM_LIVES_MS[rng.weighted_index(&[1, 3, 2])];
            Ember {
                life_ms,
                phase_ms: rng.below(life_ms),
                x0: rng.range_f64(-0.010, 0.010),
                drift: rng.range_f64(-0.035, 0.035),
                rise: rng.range_f64(0.07, 0.16),
                wobble: rng.range_f64(0.002, 0.008),
                wobble_cycles: f64::from(rng.range_u32(1, 2)),
                wobble_phase: rng.unit(),
                max_level: f64::from(rng.range_u32(4, 6)),
                glyph_bias: rng.below(2),
            }
        })
        .collect()
}

/// Offset of an ember at time `t`, in units of height, plus glyph and level.
fn ember_state(e: &Ember, t_ms: u64) -> Option<(f64, f64, u8, f64)> {
    let life = u64::from(e.life_ms);
    let age = (t_ms + u64::from(e.phase_ms)) % life;
    let p = age as f64 / life as f64;
    if p >= 0.96 {
        return None;
    }
    let dy = -e.rise * p;
    let wob = e.wobble * p * sin(TAU * (e.wobble_cycles * p + e.wobble_phase));
    let dx = e.x0 + e.drift * pow(p, 1.5) + wob;
    let level = e.max_level * pow(1.0 - p, 0.8);
    let stage = ((p * 4.0) as u32 + e.glyph_bias).min(EMBERS.len() as u32 - 1);
    Some((dx, dy, EMBERS[stage as usize], level))
}

impl Emitter {
    /// All live particles at time `t`, in pixels.
    pub fn sample(&self, t_ms: u64) -> Vec<Particle> {
        match self.kind {
            EmitterKind::Embers => self
                .embers
                .iter()
                .filter_map(|e| ember_state(e, t_ms))
                .filter_map(|(dx, dy, glyph, level)| {
                    let lv = (level * self.intensity).round();
                    if lv < 1.0 {
                        return None;
                    }
                    Some(Particle {
                        x: self.origin_x + dx * self.unit,
                        y: self.origin_y + dy * self.unit,
                        glyph,
                        level: lv.min(6.0) as u8,
                        ink: false,
                    })
                })
                .collect(),
            EmitterKind::Weather(kind) => self
                .drops
                .iter()
                .filter_map(|d| {
                    let (x, y, glyph, level) = if self.pace >= CALM_PACE {
                        calm_drop_state(kind, d, t_ms)?
                    } else {
                        drop_state(kind, d, t_ms)
                    };
                    Some(Particle {
                        x: x * self.size.0,
                        y: y * self.size.1,
                        glyph,
                        level,
                        ink: kind.ink(),
                    })
                })
                .collect(),
        }
    }
}

/// Picks the hero time: among candidate frames in the first 10 s, the one whose
/// particles are most evenly spread (lowest sum of squared bucket counts).
/// Ties go to the earliest frame. Depends only on the seed.
pub fn hero_time(embers: &[Ember]) -> u64 {
    let mut best = (u64::MAX, 0u64);
    let mut buckets: Vec<(i64, i64)> = Vec::with_capacity(embers.len());
    for k in 0..HERO_CANDIDATES {
        let t = k * HERO_STEP_MS;
        buckets.clear();
        for e in embers {
            if let Some((dx, dy, _, _)) = ember_state(e, t) {
                buckets.push((
                    (dx / HERO_BUCKET).floor() as i64,
                    (dy / HERO_BUCKET).floor() as i64,
                ));
            }
        }
        buckets.sort_unstable();
        let mut score = 0u64;
        let mut i = 0;
        while i < buckets.len() {
            let mut j = i;
            while j < buckets.len() && buckets[j] == buckets[i] {
                j += 1;
            }
            let n = (j - i) as u64;
            score += n * n;
            i = j;
        }
        if score < best.0 {
            best = (score, t);
        }
    }
    best.1
}

/// Screen-wide weather (PLAN.md section 4.2, step 6). Every motion completes a
/// whole number of cycles per [`LOOP_MS`], so weather loops seamlessly.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherKind {
    Snow,
    Rain,
    Fireflies,
    Leaves,
    Dust,
}

impl WeatherKind {
    /// The kind for a recipe's weather id (`none` and `stars` have no particles).
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "snow" => Some(WeatherKind::Snow),
            "rain" => Some(WeatherKind::Rain),
            "fireflies" => Some(WeatherKind::Fireflies),
            "leaves" => Some(WeatherKind::Leaves),
            "dust" => Some(WeatherKind::Dust),
            _ => None,
        }
    }

    /// Ink-coloured (cool) or ember-coloured (warm, glowing).
    pub fn ink(self) -> bool {
        matches!(
            self,
            WeatherKind::Snow | WeatherKind::Rain | WeatherKind::Dust
        )
    }
}

/// One weather particle, in normalised screen coordinates.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    x0: f64,
    y0: f64,
    /// Whole vertical (or horizontal, for dust) wraps per loop.
    wraps: u32,
    sway: f64,
    cycles: u32,
    phase: f64,
    glyph: u8,
    level: u8,
    /// Calm pace: how far it falls (or drifts) over one life, in screen
    /// heights (widths for dust). 0 in older scenes.
    #[serde(default)]
    fall: f64,
}

/// Calm pace: each weather particle's life, by kind (each divides LOOP_MS).
fn calm_life_ms(kind: WeatherKind) -> u64 {
    match kind {
        WeatherKind::Rain => 2000,
        _ => 6000,
    }
}

/// The weather particles for a seed; the count scales with the aspect ratio.
pub fn weather_set(kind: WeatherKind, seed: u64, aspect: f64, pace: u8) -> Vec<Drop> {
    let mut drops = old_weather_set(kind, seed, aspect);
    if pace >= CALM_PACE {
        // Separate draws, so the older pace's particles stay exactly as they
        // were.
        let mut rng = Rng::stream(seed, "weather_fall");
        for d in &mut drops {
            d.fall = match kind {
                WeatherKind::Snow => rng.range_f64(0.10, 0.22),
                WeatherKind::Rain => rng.range_f64(0.18, 0.30),
                WeatherKind::Leaves => rng.range_f64(0.12, 0.24),
                WeatherKind::Dust => rng.range_f64(0.06, 0.12),
                WeatherKind::Fireflies => 0.0,
            };
        }
    }
    drops
}

fn old_weather_set(kind: WeatherKind, seed: u64, aspect: f64) -> Vec<Drop> {
    use crate::glyphs::id;
    let mut rng = Rng::stream(seed, "weather_particles");
    let per = match kind {
        WeatherKind::Snow => 70.0,
        WeatherKind::Rain => 110.0,
        WeatherKind::Fireflies => 14.0,
        WeatherKind::Leaves => 22.0,
        WeatherKind::Dust => 40.0,
    };
    let count = (per * aspect).round() as u32;
    (0..count)
        .map(|_| {
            let x0 = rng.unit();
            let y0 = rng.unit();
            let phase = rng.unit();
            let big = rng.chance(1, 4);
            match kind {
                WeatherKind::Snow => Drop {
                    x0,
                    y0,
                    wraps: rng.range_u32(1, 2),
                    sway: rng.range_f64(0.003, 0.010),
                    cycles: rng.range_u32(1, 3),
                    phase,
                    glyph: if big {
                        id('*')
                    } else {
                        *rng.choose(&[id('.'), id('\''), id('`')])
                    },
                    level: if big { 5 } else { rng.range_u32(2, 4) as u8 },
                    fall: 0.0,
                },
                WeatherKind::Rain => Drop {
                    x0,
                    y0,
                    wraps: rng.range_u32(6, 9),
                    sway: 0.0,
                    cycles: 1,
                    phase,
                    glyph: *rng.choose(&[id('/'), id('|'), id('\'')]),
                    level: rng.range_u32(2, 3) as u8,
                    fall: 0.0,
                },
                WeatherKind::Fireflies => Drop {
                    x0,
                    // Low in the scene, among the terrain.
                    y0: 0.45 + 0.5 * y0,
                    wraps: rng.range_u32(1, 3),
                    sway: rng.range_f64(0.01, 0.03),
                    cycles: rng.range_u32(1, 2),
                    phase,
                    glyph: id('.'),
                    level: 0,
                    fall: 0.0,
                },
                WeatherKind::Leaves => Drop {
                    x0,
                    y0,
                    wraps: 1,
                    sway: rng.range_f64(0.01, 0.03),
                    cycles: rng.range_u32(2, 3),
                    phase,
                    glyph: *rng.choose(&[id(','), id('`'), id('\'')]),
                    level: rng.range_u32(2, 4) as u8,
                    fall: 0.0,
                },
                WeatherKind::Dust => Drop {
                    x0,
                    y0: 0.3 + 0.65 * y0,
                    wraps: 1,
                    sway: rng.range_f64(0.002, 0.008),
                    cycles: rng.range_u32(1, 2),
                    phase,
                    glyph: *rng.choose(&[id('.'), id('`')]),
                    level: rng.range_u32(1, 2) as u8,
                    fall: 0.0,
                },
            }
        })
        .collect()
}

fn frac(x: f64) -> f64 {
    x - x.floor()
}

/// Position (normalised), glyph and level of a weather particle at `t`.
fn drop_state(kind: WeatherKind, d: &Drop, t_ms: u64) -> (f64, f64, u8, u8) {
    use crate::glyphs::id;
    let u = (t_ms % LOOP_MS) as f64 / LOOP_MS as f64;
    let wave = |cycles: u32, phase: f64| sin(TAU * (f64::from(cycles) * u + phase));
    match kind {
        WeatherKind::Snow | WeatherKind::Leaves => (
            frac(d.x0 + d.sway * wave(d.cycles, d.phase)),
            frac(d.y0 + f64::from(d.wraps) * u),
            d.glyph,
            d.level,
        ),
        WeatherKind::Rain => (d.x0, frac(d.y0 + f64::from(d.wraps) * u), d.glyph, d.level),
        WeatherKind::Fireflies => {
            // Wander on a small closed curve and pulse.
            let glow = 0.5 + 0.5 * wave(d.wraps, d.phase * 2.0);
            let level = (1.0 + 4.0 * glow).round() as u8;
            (
                frac(d.x0 + d.sway * wave(d.cycles, d.phase)),
                d.y0 + 0.5 * d.sway * wave(d.cycles + 1, d.phase + 0.25),
                if level >= 4 { id('*') } else { d.glyph },
                level,
            )
        }
        WeatherKind::Dust => (
            frac(d.x0 + f64::from(d.wraps) * u),
            d.y0 + d.sway * wave(d.cycles, d.phase),
            d.glyph,
            d.level,
        ),
    }
}

/// A weather particle at the calm pace: it appears, drifts `fall` over its
/// life while swaying, and fades out, then starts again. `None` while fully
/// faded. Fireflies keep their wandering (already slow).
fn calm_drop_state(kind: WeatherKind, d: &Drop, t_ms: u64) -> Option<(f64, f64, u8, u8)> {
    if kind == WeatherKind::Fireflies {
        return Some(drop_state(kind, d, t_ms));
    }
    let life = calm_life_ms(kind);
    let p = frac((t_ms % life) as f64 / life as f64 + d.phase);
    // Fade in over the first fifth, out over the last fifth.
    let fade = (p / 0.2).min((1.0 - p) / 0.2).min(1.0);
    let level = (f64::from(d.level) * fade).round() as u8;
    if level == 0 {
        return None;
    }
    // One slow sway per life.
    let sway = d.sway * sin(TAU * (p + d.phase));
    Some(match kind {
        WeatherKind::Dust => (frac(d.x0 + d.fall * p), d.y0 + sway, d.glyph, level),
        WeatherKind::Rain => (d.x0, frac(d.y0 + d.fall * p), d.glyph, level),
        _ => (frac(d.x0 + sway), frac(d.y0 + d.fall * p), d.glyph, level),
    })
}

/// Particle presets attached to an anchor of the character (activity effects).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Magic: sparks flying out from a point (a wand tip).
    Sparkles,
    /// Faint wisps rising from a point (a mug, a kettle).
    Steam,
    /// Music notes floating up and sideways (a guitar).
    Notes,
    /// Snow kicked up behind skis: short arcs backwards, quick.
    SnowSpray,
}

impl Preset {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "sparkles" => Some(Preset::Sparkles),
            "steam" => Some(Preset::Steam),
            "notes" => Some(Preset::Notes),
            "snow_spray" => Some(Preset::SnowSpray),
            _ => None,
        }
    }
}

/// A preset particle: offset from its anchor in sprite pixels (y down), glyph, level 1-6.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresetParticle {
    pub dx: f64,
    pub dy: f64,
    pub glyph: u8,
    pub level: u8,
}

/// Particles of a preset at time `t`. Seeded and periodic in [`LOOP_MS`], like the
/// embers, so GIF loops stay seamless.
pub fn preset_particles(preset: Preset, seed: u64, t_ms: u64, pace: u8) -> Vec<PresetParticle> {
    use crate::glyphs::id;
    let calm = pace >= CALM_PACE;
    let (label, count, lives): (&str, u32, &[u32]) = match (preset, calm) {
        (Preset::Sparkles, false) => ("fx_sparkles", 9, &[1000, 1500, 2000]),
        (Preset::Steam, false) => ("fx_steam", 5, &[2000, 3000]),
        (Preset::Notes, false) => ("fx_notes", 4, &[1500, 2000, 3000]),
        // Calm: the same shapes, lingering about twice as long.
        (Preset::Sparkles, true) => ("fx_sparkles", 9, &[2000, 3000]),
        (Preset::Steam, true) => ("fx_steam", 5, &[3000, 6000]),
        (Preset::Notes, true) => ("fx_notes", 4, &[3000, 6000]),
        // Spray is quick even at the calm pace: it's thrown, not drifting.
        (Preset::SnowSpray, false) => ("fx_snow_spray", 8, &[600, 1000]),
        (Preset::SnowSpray, true) => ("fx_snow_spray", 8, &[1000, 1500]),
    };
    let mut rng = Rng::stream(seed, label);
    let mut out = Vec::new();
    for _ in 0..count {
        let life = *rng.choose(lives);
        let phase = rng.below(life);
        let a = rng.unit();
        let b = rng.unit();
        let c = rng.unit();
        let p = ((t_ms + u64::from(phase)) % u64::from(life)) as f64 / f64::from(life);
        match preset {
            Preset::Sparkles => {
                // Out from the tip in a random direction, fading.
                let ang = TAU * a;
                let dist = (1.0 + 5.0 * b) * p;
                let level = (6.0 * (1.0 - p)).round() as u8;
                if level == 0 {
                    continue;
                }
                let glyph = [id('*'), id('+'), id('\''), id('.')][((p * 4.0) as usize).min(3)];
                out.push(PresetParticle {
                    dx: dist * sin(ang + TAU / 4.0),
                    dy: -dist * sin(ang) - 1.5 * p,
                    glyph,
                    level,
                });
            }
            Preset::Steam => {
                // Rising and swaying, faint.
                let rise = (8.0 + 6.0 * b) * p;
                let sway = 1.2 * sin(TAU * (p + c)) * p;
                let level = (4.0 * (1.0 - p) + 0.5).round() as u8;
                if level == 0 || p < 0.08 {
                    continue;
                }
                let glyph = [id('\''), id('~'), id('`'), id('.')][((p * 4.0) as usize).min(3)];
                out.push(PresetParticle {
                    dx: sway + 0.5 * (a - 0.5),
                    dy: -rise - 1.0,
                    glyph,
                    level,
                });
            }
            Preset::SnowSpray => {
                // Backwards (negative dx: behind the skier) on a low arc.
                let back = (1.5 + 6.0 * b) * p;
                let up = (2.0 + 3.0 * a) * p - 5.0 * p * p;
                let level = (5.0 * (1.0 - p) + 0.5).round() as u8;
                if level == 0 {
                    continue;
                }
                let glyph = [id('*'), id('\''), id('.'), id(',')][((p * 4.0) as usize).min(3)];
                out.push(PresetParticle {
                    dx: -back - 1.0 - c,
                    dy: -up,
                    glyph,
                    level,
                });
            }
            Preset::Notes => {
                // Up and out to one side, swaying, fading.
                let side = if a < 0.5 { -1.0 } else { 1.0 };
                let rise = (6.0 + 5.0 * b) * p;
                let level = (5.0 * (1.0 - p) + 0.5).round() as u8;
                if level == 0 {
                    continue;
                }
                out.push(PresetParticle {
                    dx: side * (1.0 + 3.0 * p) + 0.8 * sin(TAU * (2.0 * p + c)),
                    dy: -rise - 1.0,
                    glyph: crate::glyphs::NOTES[(b * 2.0) as usize % 2],
                    level,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod weather_tests {
    use super::*;

    const KINDS: [WeatherKind; 5] = [
        WeatherKind::Snow,
        WeatherKind::Rain,
        WeatherKind::Fireflies,
        WeatherKind::Leaves,
        WeatherKind::Dust,
    ];

    fn emitter(kind: WeatherKind, pace: u8) -> Emitter {
        Emitter {
            kind: EmitterKind::Weather(kind),
            origin_x: 0.0,
            origin_y: 0.0,
            unit: 1.0,
            intensity: 1.0,
            embers: Vec::new(),
            drops: weather_set(kind, 9, 16.0 / 9.0, pace),
            size: (1920.0, 1080.0),
            pace,
        }
    }

    #[test]
    fn weather_loops_and_moves() {
        for pace in [0, CALM_PACE] {
            for kind in KINDS {
                let e = emitter(kind, pace);
                assert!(!e.drops.is_empty());
                assert_eq!(
                    e.sample(1234),
                    e.sample(1234 + LOOP_MS),
                    "{kind:?} pace {pace}"
                );
                assert_ne!(
                    e.sample(0),
                    e.sample(700),
                    "{kind:?} pace {pace} should move"
                );
            }
        }
    }

    /// Mean speed of the particles that move smoothly between `t` and
    /// `t + 100 ms` (respawns excluded), in screen heights per second.
    fn mean_speed(kind: WeatherKind, pace: u8) -> f64 {
        let e = emitter(kind, pace);
        let (mut sum, mut n) = (0.0, 0usize);
        for k in 0..60u64 {
            let t = k * 100;
            for d in &e.drops {
                let at = |t| match pace {
                    0 => Some(drop_state(kind, d, t)),
                    _ => calm_drop_state(kind, d, t),
                };
                if let (Some(a), Some(b)) = (at(t), at(t + 100)) {
                    let (dx, dy) = ((b.0 - a.0) * 16.0 / 9.0, b.1 - a.1);
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 0.3 {
                        sum += dist * 10.0;
                        n += 1;
                    }
                }
            }
        }
        sum / n.max(1) as f64
    }

    #[test]
    fn the_calm_pace_is_calm() {
        for kind in KINDS {
            let (old, calm) = (mean_speed(kind, 0), mean_speed(kind, CALM_PACE));
            let cap = if kind == WeatherKind::Rain { 0.2 } else { 0.06 };
            assert!(calm <= cap, "{kind:?}: {calm:.3} screen heights a second");
            if kind != WeatherKind::Fireflies {
                assert!(calm < old * 0.6, "{kind:?}: calm {calm:.3} vs {old:.3}");
            }
        }
    }

    #[test]
    fn the_original_pace_is_unchanged() {
        // Older scenes keep their motion: no `fall`, the original formula.
        let e = emitter(WeatherKind::Snow, 0);
        assert!(e.drops.iter().all(|d| d.fall == 0.0));
        let calm = emitter(WeatherKind::Snow, CALM_PACE);
        assert_eq!(
            e.drops.iter().map(|d| (d.x0, d.y0)).collect::<Vec<_>>(),
            calm.drops.iter().map(|d| (d.x0, d.y0)).collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn presets_loop_and_vary() {
        for pace in [0, CALM_PACE] {
            for preset in [Preset::Sparkles, Preset::Steam, Preset::Notes] {
                assert_eq!(
                    preset_particles(preset, 3, 700, pace),
                    preset_particles(preset, 3, 700 + LOOP_MS, pace)
                );
                assert_ne!(
                    preset_particles(preset, 3, 700, pace),
                    preset_particles(preset, 3, 1900, pace)
                );
                assert!(
                    !preset_particles(preset, 3, 0, pace).is_empty()
                        || !preset_particles(preset, 3, 500, pace).is_empty()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embers(pace: u8) -> Emitter {
        Emitter {
            kind: EmitterKind::Embers,
            origin_x: 100.0,
            origin_y: 200.0,
            unit: 1000.0,
            intensity: 1.0,
            embers: ember_set(5, pace),
            drops: Vec::new(),
            size: (0.0, 0.0),
            pace,
        }
    }

    #[test]
    fn particles_loop_seamlessly() {
        for pace in [0, CALM_PACE] {
            let e = embers(pace);
            assert_eq!(e.sample(1234), e.sample(1234 + LOOP_MS));
            assert_eq!(e.sample(0), e.sample(LOOP_MS * 3));
        }
    }

    #[test]
    fn calm_embers_float_up_slowly() {
        // The fastest ember's rise, in image heights a second.
        let fastest = |set: &[Ember]| {
            set.iter()
                .map(|e| e.rise / (f64::from(e.life_ms) / 1000.0))
                .fold(0.0, f64::max)
        };
        let (old, calm) = (fastest(&ember_set(5, 0)), fastest(&ember_set(5, CALM_PACE)));
        assert!(calm <= 0.08, "calm embers rise {calm:.3} heights a second");
        assert!(
            old > 0.2,
            "the original pace is kept for older scenes ({old:.3})"
        );
    }

    #[test]
    fn hero_time_is_stable_and_in_range() {
        for pace in [0, CALM_PACE] {
            let set = ember_set(5, pace);
            let t = hero_time(&set);
            assert_eq!(t, hero_time(&ember_set(5, pace)));
            assert!(t < HERO_STEP_MS * HERO_CANDIDATES);
        }
    }
}
