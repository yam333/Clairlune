//! The seeded activity state machine.
//!
//! The state at time `t` is a pure function of `(activity, seed, t)`. To keep that
//! cheap at any `t`, time is split into 60 s episodes. Each episode is seeded by
//! `hash(seed, "animation", episode)`, starts in the `start` state and is simulated
//! from its own beginning, so a lookup costs at most one episode of transitions.

use crate::activity::Activity;
use crate::hash::StableHasher;
use crate::rng::Rng;

/// Episode length.
pub const EPISODE_MS: u64 = 60_000;
/// One-shot state length when no sprite sheet gives one (placeholder art).
pub const DEFAULT_ONE_SHOT_MS: u64 = 1_000;

/// The state active at some time, and the span it occupies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateAt {
    pub state: String,
    pub since_ms: u64,
    pub until_ms: u64,
}

/// State at `t_ms`. `one_shot_ms` gives the play length of non-looping states
/// (the sum of their sprite frame durations).
pub fn state_at(
    activity: &Activity,
    seed: u64,
    t_ms: u64,
    one_shot_ms: &dyn Fn(&str) -> u64,
) -> StateAt {
    let episode = t_ms / EPISODE_MS;
    let mut h = StableHasher::new();
    h.write_u64(seed);
    h.write_str("animation");
    h.write_u64(episode);
    let mut rng = Rng::from_u64(h.finish());

    let mut state = activity.animation.start.as_str();
    let mut since = episode * EPISODE_MS;
    // Saturating: the last episode before u64::MAX is cut short.
    let end = since.saturating_add(EPISODE_MS);
    loop {
        let duration = match activity.animation.states.get(state) {
            Some(def) if def.looping => {
                let [lo, hi] = def.dwell_s.unwrap_or([1.0, 1.0]);
                let (lo, hi) = ((lo * 1000.0).round() as u32, (hi * 1000.0).round() as u32);
                u64::from(rng.range_u32(lo, hi.max(lo)))
            }
            _ => one_shot_ms(state).max(1),
        };
        // The episode boundary cuts the last state short; the next episode restarts.
        let until = since.saturating_add(duration).min(end);
        if t_ms < until || until == end {
            return StateAt {
                state: state.to_string(),
                since_ms: since,
                until_ms: until,
            };
        }
        since = until;
        let successors = activity.successors(state);
        let weights: Vec<u32> = successors.iter().map(|(_, w)| *w).collect();
        state = successors[rng.weighted_index(&weights)].0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::Catalog;

    fn one_shot(_: &str) -> u64 {
        DEFAULT_ONE_SHOT_MS
    }

    #[test]
    fn starts_in_start_and_restarts_each_episode() {
        let cat = Catalog::builtin();
        for a in &cat.activities {
            for ep in 0..3 {
                let s = state_at(a, 7, ep * EPISODE_MS, &one_shot);
                assert_eq!(s.state, a.animation.start, "{}", a.id);
                assert_eq!(s.since_ms, ep * EPISODE_MS);
            }
        }
    }

    #[test]
    fn the_end_of_time_is_still_a_state() {
        let cat = Catalog::builtin();
        for a in &cat.activities {
            for t in [u64::MAX, u64::MAX - 1, u64::MAX - EPISODE_MS] {
                let s = state_at(a, 7, t, &one_shot);
                assert!(s.since_ms <= t && t <= s.until_ms, "{}", a.id);
            }
        }
    }

    #[test]
    fn walks_only_valid_transitions_and_is_deterministic() {
        let cat = Catalog::builtin();
        for a in &cat.activities {
            let mut prev: Option<StateAt> = None;
            let mut t = 0;
            while t < 3 * EPISODE_MS {
                let s = state_at(a, 42, t, &one_shot);
                assert_eq!(s, state_at(a, 42, t, &one_shot));
                assert!(a.animation.states.contains_key(&s.state));
                assert!(s.since_ms <= t && t < s.until_ms);
                if let Some(p) = &prev
                    && p != &s
                    && !s.since_ms.is_multiple_of(EPISODE_MS)
                {
                    assert_eq!(p.until_ms, s.since_ms);
                    assert!(a.successors(&p.state).iter().any(|(n, _)| *n == s.state));
                }
                prev = Some(s);
                t += 250;
            }
        }
    }

    #[test]
    fn looping_dwell_respects_bounds() {
        let cat = Catalog::builtin();
        let a = cat.get("warm_hands").unwrap();
        let s = state_at(a, 3, 0, &one_shot);
        let [lo, hi] = a.animation.states[&s.state].dwell_s.unwrap();
        let len = s.until_ms - s.since_ms;
        assert!(
            len >= (lo * 1000.0) as u64 && len <= (hi * 1000.0) as u64,
            "{len}"
        );
    }
}
