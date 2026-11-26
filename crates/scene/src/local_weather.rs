//! Weather a landscape has of its own: sea spray at the shore and the
//! lighthouse, sandstorms and tumbleweed in dry country, cinders over the
//! volcano, sky lanterns and fireworks over water and town, glowing spores
//! and wisps in damp places, bats about old stones, spindrift off the peaks
//! and dragonflies over still water. Each is listed only by the landscapes
//! it belongs to (`archetype::LOCAL_WEATHERS`).
//!
//! Like all weather, every motion is periodic in [`LOOP_MS`], so GIFs loop.
//! These came after the calm pace, so they only have that one.

use crate::glyphs::id;
use crate::math::{TAU, pow, sin, smoothstep};
use crate::particles::{Drop, LOOP_MS, WeatherKind};
use crate::rng::Rng;

/// The particles of a landscape's own weather, or `None` for other kinds.
pub(crate) fn set(kind: WeatherKind, seed: u64, aspect: f64) -> Option<Vec<Drop>> {
    use WeatherKind as K;
    let per = match kind {
        K::Spray => 70.0,
        K::Sandstorm => 150.0,
        K::Tumbleweed => 1.6,
        K::Cinders => 70.0,
        K::SkyLanterns => 14.0,
        K::Fireworks => return Some(fireworks(seed, aspect)),
        K::Spores => 50.0,
        K::Wisps => 4.0,
        K::Bats => 7.0,
        K::Spindrift => 110.0,
        K::Dragonflies => 8.0,
        _ => return None,
    };
    let mut rng = Rng::stream(seed, "local_weather");
    let count = ((per * aspect).round() as u32).max(1);
    let drops = (0..count)
        .map(|_| {
            let x0 = rng.unit();
            let y = rng.unit();
            let phase = rng.unit();
            let big = rng.chance(1, 4);
            let (y0, sway, fall, glyph, level) = match kind {
                // Thrown up off the water low in the scene, arcing back.
                K::Spray => (
                    0.76 + 0.21 * y,
                    rng.range_f64(-0.012, 0.012),
                    rng.range_f64(0.035, 0.09),
                    *rng.choose(&[id('.'), id('\''), id('*'), id('*')]),
                    rng.range_u32(3, 5),
                ),
                // Long streaks of sand driven across, thick.
                K::Sandstorm => (
                    0.22 + 0.75 * y,
                    rng.range_f64(0.002, 0.008),
                    rng.range_f64(0.35, 0.6),
                    *rng.choose(&[id('-'), id('~'), id('.'), id('`'), id('='), id('-')]),
                    rng.range_u32(2, if big { 5 } else { 4 }),
                ),
                // A bundle bowling along the ground.
                K::Tumbleweed => (0.74 + 0.16 * y, 0.0, rng.range_f64(0.5, 0.8), id('@'), 4),
                // Sparks rising from low down, cooling as they go.
                K::Cinders => (
                    0.55 + 0.45 * y,
                    rng.range_f64(0.006, 0.016),
                    rng.range_f64(0.25, 0.45),
                    id('*'),
                    rng.range_u32(4, 6),
                ),
                // Lanterns rising slowly from the water and the land.
                K::SkyLanterns => (
                    0.55 + 0.35 * y,
                    rng.range_f64(0.004, 0.01),
                    rng.range_f64(0.3, 0.5),
                    if !rng.chance(1, 3) { id('O') } else { id('o') },
                    rng.range_u32(5, 6),
                ),
                // Tiny lights drifting up, pulsing.
                K::Spores => (
                    0.4 + 0.55 * y,
                    rng.range_f64(0.006, 0.02),
                    rng.range_f64(0.06, 0.14),
                    *rng.choose(&[id('.'), id('\''), id('o'), id('.')]),
                    rng.range_u32(3, 5),
                ),
                // A few lights low over the ground, wandering, coming and
                // going.
                K::Wisps => (0.55 + 0.35 * y, rng.range_f64(0.02, 0.05), 0.0, id('o'), 5),
                // Flitting across the sky, up and down.
                K::Bats => (
                    0.1 + 0.35 * y,
                    0.0,
                    rng.range_f64(0.3, 0.5),
                    id('v'),
                    rng.range_u32(4, 5),
                ),
                // Snow blown sideways off the ridges, high up.
                K::Spindrift => (
                    0.12 + 0.4 * y,
                    rng.range_f64(0.003, 0.01),
                    rng.range_f64(0.25, 0.45),
                    *rng.choose(&[id('.'), id('\''), id('-'), id('`')]),
                    rng.range_u32(2, 4),
                ),
                // Darting and hovering low over the land and water.
                K::Dragonflies => (0.45 + 0.4 * y, 0.0, 0.0, id('+'), rng.range_u32(5, 6)),
                _ => unreachable!("not a landscape's own weather"),
            };
            Drop {
                x0,
                y0,
                wraps: 1,
                sway,
                cycles: 0,
                phase,
                glyph,
                level: level as u8,
                fall,
            }
        })
        .collect();
    Some(drops)
}

/// Fireworks: two or three bursts across the sky, each a rocket going up
/// and then a ring of sparks opening and drooping. A spark's `sway` and
/// `fall` are where it ends up (across in screen widths, so the ring is
/// round on screen); `cycles` is 1 for the rocket.
fn fireworks(seed: u64, aspect: f64) -> Vec<Drop> {
    let mut rng = Rng::stream(seed, "fireworks");
    let bursts = (1.4 * aspect).round().max(1.0) as u32;
    let mut drops = Vec::new();
    for _ in 0..bursts {
        let x0 = rng.range_f64(0.08, 0.92);
        let y0 = rng.range_f64(0.08, 0.32);
        let phase = rng.unit();
        let r = rng.range_f64(0.06, 0.1);
        drops.push(Drop {
            x0,
            y0,
            wraps: 1,
            sway: 0.0,
            cycles: 1,
            phase,
            glyph: id('\''),
            level: 3,
            fall: 0.0,
        });
        let n = rng.range_u32(14, 20);
        let turn = rng.unit();
        for i in 0..n {
            let a = TAU * (f64::from(i) / f64::from(n) + turn);
            let reach = r * rng.range_f64(0.8, 1.05);
            drops.push(Drop {
                x0,
                y0,
                wraps: 1,
                sway: sin(a + TAU / 4.0) * reach / aspect,
                cycles: 0,
                phase,
                glyph: id('*'),
                level: rng.range_u32(5, 6) as u8,
                fall: sin(a) * reach,
            });
        }
    }
    drops
}

fn frac(x: f64) -> f64 {
    x - x.floor()
}

/// Where a particle of a landscape's own weather is at `t`: `None` for other
/// kinds, `Some(None)` while it isn't showing.
#[allow(clippy::option_option)]
pub(crate) fn state(kind: WeatherKind, d: &Drop, t_ms: u64) -> Option<Option<(f64, f64, u8, u8)>> {
    use WeatherKind as K;
    let life = match kind {
        K::Spray | K::Sandstorm | K::Spindrift => 2000,
        K::Tumbleweed
        | K::Cinders
        | K::SkyLanterns
        | K::Fireworks
        | K::Spores
        | K::Wisps
        | K::Bats
        | K::Dragonflies => LOOP_MS,
        _ => return None,
    };
    let p = frac((t_ms % life) as f64 / life as f64 + d.phase);
    // Where the whole loop is, for gusts that come and go together.
    let u = (t_ms % LOOP_MS) as f64 / LOOP_MS as f64;
    let fade = (p / 0.2).min((1.0 - p) / 0.2).min(1.0);
    let lv = |k: f64| -> Option<u8> {
        let l = (f64::from(d.level) * k).round();
        (l >= 1.0).then_some(l.min(6.0) as u8)
    };
    let wave = |cycles: f64, phase: f64| sin(TAU * (cycles * p + phase));
    let shown = match kind {
        K::Spray => {
            let up = 4.0 * p * (1.0 - p);
            let glyph = if p < 0.5 { d.glyph } else { id('.') };
            lv(fade).map(|l| (frac(d.x0 + d.sway * p), d.y0 - d.fall * up, glyph, l))
        }
        K::Sandstorm => {
            let gust = 0.55 + 0.45 * sin(TAU * (u + d.y0 * 0.5));
            lv(fade * gust).map(|l| {
                (
                    frac(d.x0 + d.fall * p),
                    d.y0 + d.sway * wave(2.0, d.phase),
                    d.glyph,
                    l,
                )
            })
        }
        K::Tumbleweed => {
            const ROLL: [u8; 4] = [id('@'), id('%'), id('&'), id('#')];
            let bounce = wave(3.0, d.phase).abs();
            Some((
                frac(d.x0 + d.fall * p),
                d.y0 - 0.025 * bounce,
                ROLL[((p * 24.0) as usize) % 4],
                d.level,
            ))
        }
        K::Cinders => {
            let glyph = if p < 0.3 {
                id('*')
            } else if p < 0.65 {
                id('+')
            } else {
                id('.')
            };
            lv(fade * (1.0 - 0.4 * p)).map(|l| {
                (
                    frac(d.x0 + d.sway * wave(2.0, d.phase)),
                    d.y0 - d.fall * p,
                    glyph,
                    l,
                )
            })
        }
        K::SkyLanterns => {
            let flicker = 0.85 + 0.15 * wave(30.0, d.phase);
            lv(fade * flicker).map(|l| {
                (
                    frac(d.x0 + d.sway * wave(1.0, d.phase) + 0.04 * p),
                    d.y0 - d.fall * p,
                    d.glyph,
                    l,
                )
            })
        }
        K::Fireworks => burst(d, p),
        K::Spores => {
            let glow = 0.6 + 0.4 * wave(3.0, d.phase);
            lv(fade * glow).map(|l| {
                (
                    frac(d.x0 + d.sway * wave(1.5, d.phase)),
                    d.y0 - d.fall * p,
                    d.glyph,
                    l,
                )
            })
        }
        K::Wisps => {
            let glow = wave(1.0, d.phase).max(0.0);
            lv(glow * glow).map(|l| {
                (
                    frac(d.x0 + d.sway * wave(1.0, d.phase * 0.7)),
                    d.y0 + 0.4 * d.sway * wave(2.0, d.phase),
                    if l >= 4 { id('*') } else { d.glyph },
                    l,
                )
            })
        }
        K::Bats => {
            let flap = ((p * 120.0) as usize).is_multiple_of(2);
            lv(fade).map(|l| {
                (
                    frac(d.x0 + d.fall * p + 0.02 * wave(3.0, d.phase)),
                    d.y0 + 0.03 * wave(5.0, d.phase) + 0.012 * wave(13.0, 0.0),
                    if flap { id('v') } else { id('^') },
                    l,
                )
            })
        }
        K::Spindrift => {
            // Blown off in gusts: thick, then hardly any.
            let gust = 0.5 + 0.5 * sin(TAU * u);
            lv(fade * (0.15 + 0.85 * gust)).map(|l| {
                (
                    frac(d.x0 + d.fall * p),
                    d.y0 - 0.01 * p + d.sway * wave(1.0, d.phase),
                    d.glyph,
                    l,
                )
            })
        }
        K::Dragonflies => {
            // Four darts a loop, hovering between them.
            let target = |j: i64| {
                let j = j.rem_euclid(4) as f64;
                (
                    d.x0 + 0.07 * sin(TAU * (j * 0.37 + d.phase)),
                    d.y0 + 0.035 * sin(TAU * (j * 0.61 + d.phase * 1.7)),
                )
            };
            let k = (p * 4.0).floor() as i64;
            let m = smoothstep(0.0, 0.18, frac(p * 4.0));
            let ((ax, ay), (bx, by)) = (target(k - 1), target(k));
            let wings = ((p * 200.0) as usize).is_multiple_of(2);
            Some((
                frac(ax + (bx - ax) * m),
                ay + (by - ay) * m,
                if wings { id('+') } else { id('=') },
                d.level,
            ))
        }
        _ => unreachable!("matched above"),
    };
    Some(shown)
}

/// A firework's drop at `p` through its life: the rocket going up, then
/// the sparks opening out, drooping and fading; nothing for the rest.
fn burst(d: &Drop, p: f64) -> Option<(f64, f64, u8, u8)> {
    const UP: f64 = 0.08;
    const OPEN: f64 = 0.34;
    if d.cycles == 1 {
        return (p < UP).then(|| (d.x0, d.y0 + 0.22 * (1.0 - p / UP), d.glyph, d.level));
    }
    if !(UP..UP + OPEN).contains(&p) {
        return None;
    }
    let s = (p - UP) / OPEN;
    let out = 1.0 - (1.0 - s) * (1.0 - s);
    let glyph = if s < 0.25 {
        id('*')
    } else if s < 0.6 {
        id('+')
    } else {
        id('.')
    };
    let level = (f64::from(d.level) * pow(1.0 - s, 0.6)).round();
    (level >= 1.0).then(|| {
        (
            frac(d.x0 + d.sway * out),
            d.y0 + d.fall * out + 0.05 * s * s,
            glyph,
            level as u8,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL: [WeatherKind; 11] = [
        WeatherKind::Spray,
        WeatherKind::Sandstorm,
        WeatherKind::Tumbleweed,
        WeatherKind::Cinders,
        WeatherKind::SkyLanterns,
        WeatherKind::Fireworks,
        WeatherKind::Spores,
        WeatherKind::Wisps,
        WeatherKind::Bats,
        WeatherKind::Spindrift,
        WeatherKind::Dragonflies,
    ];

    #[test]
    fn every_local_weather_has_particles_that_show_and_loop() {
        for kind in LOCAL {
            let drops = set(kind, 7, 16.0 / 9.0).expect("a local kind");
            assert!(!drops.is_empty(), "{kind:?}");
            let shown = |t| {
                drops
                    .iter()
                    .filter_map(|d| state(kind, d, t).expect("a local kind"))
                    .collect::<Vec<_>>()
            };
            // Something shows over the loop, inside the screen across.
            let mut any = false;
            for t in (0..LOOP_MS).step_by(250) {
                for (x, _, g, l) in shown(t) {
                    assert!((0.0..1.0).contains(&x), "{kind:?} at {t}: x {x}");
                    assert!((1..=6).contains(&l), "{kind:?} level {l}");
                    assert!(usize::from(g) < crate::glyphs::GLYPHS.len());
                    any = true;
                }
            }
            assert!(any, "{kind:?} never shows");
            // The loop closes: the same at its start and a loop later.
            assert_eq!(shown(1234), shown(1234 + LOOP_MS), "{kind:?}");
        }
        assert!(set(WeatherKind::Snow, 7, 1.0).is_none());
    }

    #[test]
    fn a_firework_goes_up_then_opens_into_a_ring() {
        let drops = fireworks(3, 16.0 / 9.0);
        let rocket = drops.iter().find(|d| d.cycles == 1).unwrap();
        let start = (1.0 - rocket.phase) * LOOP_MS as f64;
        let at = |frac: f64| ((start + frac * LOOP_MS as f64) as u64) % LOOP_MS;
        // Going up: the rocket shows, below where it bursts.
        let (_, y, _, _) = state(WeatherKind::Fireworks, rocket, at(0.04))
            .unwrap()
            .unwrap();
        assert!(y > rocket.y0);
        // Open: sparks of that burst all round its centre.
        let sparks: Vec<_> = drops
            .iter()
            .filter(|d| d.cycles == 0 && d.phase == rocket.phase)
            .filter_map(|d| state(WeatherKind::Fireworks, d, at(0.2)).unwrap())
            .collect();
        assert!(sparks.len() > 10);
        assert!(sparks.iter().any(|s| s.0 < rocket.x0) && sparks.iter().any(|s| s.0 > rocket.x0));
        // Later in the loop the sky is clear of it.
        assert!(
            state(WeatherKind::Fireworks, rocket, at(0.8))
                .unwrap()
                .is_none()
        );
    }
}
