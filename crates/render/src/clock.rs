//! Which moment of the character's life a frame shows.
//!
//! - `Clock::Live`: the seeded state machine plays freely;
//!   live mode never repeats exactly. Particles sit on the cell grid: stills,
//!   thumbnails and the CPU fallback.
//! - `Clock::Smooth`: the same, with particles at sub-cell pixel positions,
//!   so a higher frame rate moves them smoothly instead of a cell at a time:
//!   live playback on a GPU.
//! - `Clock::Loop(loop_ms)`: a seamless loop for GIF export. The character follows
//!   the activity's `gif.loop_script`, its first looping state repeated to fill
//!   the loop, then time-scaled so the script ends exactly at `loop_ms`; pacing
//!   is snapped to a period that divides the loop. Frame `t = loop_ms` equals
//!   frame `t = 0`.

use clairlune_scene::activity::Activity;
use clairlune_scene::anim::DEFAULT_ONE_SHOT_MS;
use clairlune_scene::{CharacterTrack, Motion};
use clairlune_sprites::Character;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    Live,
    Smooth,
    Loop(u64),
}

/// One state of the loop script, placed on the loop's timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Segment {
    state: String,
    start: u64,
    len: u64,
    /// The state's own animation length (one cycle), in animation time.
    cycle: u64,
}

/// The character's timeline for one seamless loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopScript {
    loop_ms: u64,
    segments: Vec<Segment>,
}

impl LoopScript {
    pub fn new(activity: &Activity, character: &dyn Character, loop_ms: u64) -> Self {
        let script: Vec<String> = activity
            .gif
            .as_ref()
            .map(|g| g.loop_script.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| vec![activity.animation.start.clone()]);
        let cycle = |s: &str| {
            character
                .animation(&activity.id, s)
                .map(|a| a.duration_ms())
                .unwrap_or(DEFAULT_ONE_SHOT_MS)
                .max(1)
        };
        let looping = |s: &str| activity.animation.states.get(s).is_some_and(|d| d.looping);

        // Repeat the first looping state's cycle while it still fits in the loop.
        let base: u64 = script.iter().map(|s| cycle(s)).sum();
        let rep = script.iter().position(|s| looping(s)).unwrap_or(0);
        let rep_cycle = cycle(&script[rep]);
        let mut extra = 0;
        while base + (extra + 1) * rep_cycle <= loop_ms {
            extra += 1;
        }
        let mut natural: Vec<(String, u64)> = Vec::new();
        for (i, s) in script.iter().enumerate() {
            let n = if i == rep { 1 + extra } else { 1 };
            for _ in 0..n {
                natural.push((s.clone(), cycle(s)));
            }
        }

        // Time-scale onto [0, loop_ms) with integer boundaries.
        let total: u64 = natural.iter().map(|(_, c)| c).sum::<u64>().max(1);
        let mut segments = Vec::with_capacity(natural.len());
        let mut cum = 0u64;
        for (state, c) in natural {
            let start = cum * loop_ms / total;
            cum += c;
            let end = cum * loop_ms / total;
            if end > start {
                segments.push(Segment {
                    state,
                    start,
                    len: end - start,
                    cycle: c,
                });
            }
        }
        Self { loop_ms, segments }
    }

    /// State and elapsed animation time at `t` (wraps every `loop_ms`).
    pub fn at(&self, t_ms: u64) -> (&str, u64) {
        let t = t_ms % self.loop_ms.max(1);
        let seg = self
            .segments
            .iter()
            .rev()
            .find(|s| s.start <= t)
            .unwrap_or(&self.segments[0]);
        let elapsed = (t - seg.start) * seg.cycle / seg.len.max(1);
        (&seg.state, elapsed.min(seg.cycle - 1))
    }
}

/// The character track with pacing snapped to a period that divides the loop.
pub fn loop_track(track: CharacterTrack, loop_ms: u64) -> CharacterTrack {
    match track.motion {
        Motion::Pace {
            x_min,
            x_max,
            period_ms,
        } => {
            // Whole round trips per loop, nearest to the natural speed, dividing the loop.
            let target = (loop_ms as f64 / period_ms.max(1) as f64).round().max(1.0) as u64;
            let n = (1..=loop_ms)
                .filter(|n| loop_ms.is_multiple_of(*n))
                .min_by_key(|n| n.abs_diff(target))
                .unwrap_or(1);
            CharacterTrack {
                motion: Motion::Pace {
                    x_min,
                    x_max,
                    period_ms: loop_ms / n,
                },
                ..track
            }
        }
        Motion::Fly {
            x_min,
            x_max,
            period_ms,
            lift,
            bob,
            bob_ms,
        } => {
            let Motion::Pace { period_ms, .. } = loop_track(
                CharacterTrack {
                    motion: Motion::Pace {
                        x_min,
                        x_max,
                        period_ms,
                    },
                    ..track
                },
                loop_ms,
            )
            .motion
            else {
                unreachable!("pacing stays pacing")
            };
            // The bob too: whole periods per loop.
            let n = (loop_ms / bob_ms.max(1)).max(1);
            let bob_ms = (1..=n)
                .rev()
                .find(|k| loop_ms.is_multiple_of(*k))
                .map_or(loop_ms, |k| loop_ms / k);
            CharacterTrack {
                motion: Motion::Fly {
                    x_min,
                    x_max,
                    period_ms,
                    lift,
                    bob,
                    bob_ms,
                },
                ..track
            }
        }
        Motion::Stationary { .. } => track,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_scene::activity::builtin_catalog;
    use clairlune_sprites::builtin_character;

    #[test]
    fn script_fills_the_loop_and_wraps() {
        for a in &builtin_catalog().activities {
            for loop_ms in [6000, 12000] {
                let s = LoopScript::new(a, builtin_character(), loop_ms);
                assert_eq!(s.segments.first().unwrap().start, 0, "{}", a.id);
                let last = s.segments.last().unwrap();
                assert_eq!(last.start + last.len, loop_ms, "{}", a.id);
                assert_eq!(s.at(0), s.at(loop_ms));
                // Starts on the first frame of the script's first state.
                assert_eq!(s.at(0).1, 0);
            }
        }
    }

    #[test]
    fn pacing_divides_the_loop() {
        let track = CharacterTrack {
            y: 0,
            scale: 1,
            face_left: false,
            motion: Motion::Pace {
                x_min: 0,
                x_max: 100,
                period_ms: 4700,
            },
        };
        let t = loop_track(track, 6000);
        let Motion::Pace { period_ms, .. } = t.motion else {
            panic!()
        };
        assert!(6000u64.is_multiple_of(period_ms));
        assert_eq!(t.at(0), t.at(6000));
    }

    #[test]
    fn flying_divides_the_loop() {
        let track = CharacterTrack {
            y: 500,
            scale: 2,
            face_left: false,
            motion: Motion::Fly {
                x_min: 0,
                x_max: 900,
                period_ms: 13_300,
                lift: 200,
                bob: 6,
                bob_ms: 3000,
            },
        };
        for loop_ms in [6000, 12000] {
            let t = loop_track(track, loop_ms);
            assert_eq!(t.at(0), t.at(loop_ms));
            assert_eq!(t.at(1234), t.at(1234 + loop_ms));
        }
    }
}
