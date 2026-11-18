//! Milestone M1 placeholder pixel maps, taken from the Python prototype
//! (PLAN.md section 13). Milestone M2 replaces them with project-drawn sprite
//! sheets behind a `Character` trait in a `sprites` crate.
//!
//! `CLAWD_PLACEHOLDER` depicts Clawd, Anthropic's character. It is fan art and is
//! NOT covered by this crate's MIT/Apache-2.0 licence; see the NOTICE file. To
//! remove it (docs/takedown.md), delete the constant and render with Buddy off.

use crate::palette::Rgb;

/// A tiny pixel-art image: one string per row, `.` is transparent.
#[derive(Clone, Copy, Debug)]
pub struct PixelMap {
    pub width: u32,
    pub height: u32,
    pub rows: &'static [&'static str],
    pub colors: &'static [(char, Rgb)],
}

impl PixelMap {
    /// Colour of pixel `(x, y)`, or `None` when transparent.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgb> {
        let ch = self.rows[y as usize].as_bytes()[x as usize] as char;
        if ch == '.' {
            return None;
        }
        self.colors
            .iter()
            .find(|(c, _)| *c == ch)
            .map(|(_, rgb)| *rgb)
    }

    /// Every distinct colour the map uses, in table order.
    pub fn palette(&self) -> impl Iterator<Item = Rgb> + '_ {
        self.colors.iter().map(|(_, rgb)| *rgb)
    }
}

const CLAWD_BODY: Rgb = Rgb::hex(0xD9785C);
const CLAWD_EYES: Rgb = Rgb::hex(0x261A16);

/// The prototype's 15x9 Clawd (`X` body, `E` eyes). Fan art: see the module docs.
pub const CLAWD_PLACEHOLDER: PixelMap = PixelMap {
    width: 15,
    height: 9,
    rows: &[
        "..XXXXXXXXXXX..",
        "..XXXXXXXXXXX..",
        "..XXEXXXXXEXX..",
        "XXXXEXXXXXEXXXX",
        "XXXXXXXXXXXXXXX",
        "..XXXXXXXXXXX..",
        "..XXXXXXXXXXX..",
        "..X.X.....X.X..",
        "..X.X.....X.X..",
    ],
    colors: &[('X', CLAWD_BODY), ('E', CLAWD_EYES)],
};

const FIRE_ORANGE: Rgb = Rgb::hex(0xE86028);
const FIRE_YELLOW: Rgb = Rgb::hex(0xFFAA3C);
const FIRE_WHITE: Rgb = Rgb::hex(0xFFECAA);
const LOG: Rgb = Rgb::hex(0x784E30);
const LOG_DARK: Rgb = Rgb::hex(0x462E1E);

/// The prototype's 9x11 campfire.
pub const CAMPFIRE: PixelMap = PixelMap {
    width: 9,
    height: 11,
    rows: &[
        "....Y....",
        "...OY....",
        "..OOYO...",
        "..OYYO...",
        ".OOYYYO..",
        ".OYYWYYO.",
        "OOYYWWYOO",
        "OYYWWWYYO",
        ".OYYYYYO.",
        "BBBBBBBBB",
        "KKKKKKKKK",
    ],
    colors: &[
        ('O', FIRE_ORANGE),
        ('Y', FIRE_YELLOW),
        ('W', FIRE_WHITE),
        ('B', LOG),
        ('K', LOG_DARK),
    ],
};

/// A log seat by the fire: the unattended variant used when Buddy is off, so the
/// composition keeps an object where Clawd would stand.
pub const LOG_SEAT: PixelMap = PixelMap {
    width: 13,
    height: 4,
    rows: &[
        ".BBBBBBBBBBB.",
        "KBBBBBBBBBBBB",
        "BBBBBBBBBBBKB",
        ".KKKKKKKKKKK.",
    ],
    colors: &[('B', LOG), ('K', LOG_DARK)],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_are_rectangular_and_fully_coloured() {
        for map in [CLAWD_PLACEHOLDER, CAMPFIRE, LOG_SEAT] {
            assert_eq!(map.rows.len() as u32, map.height);
            for row in map.rows {
                assert_eq!(row.len() as u32, map.width);
                for ch in row.chars() {
                    assert!(
                        ch == '.' || map.colors.iter().any(|(c, _)| *c == ch),
                        "{ch}"
                    );
                }
            }
        }
    }
}
