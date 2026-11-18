//! The global glyph table and the glyph families used by layers.
//!
//! A cell stores a `u8` index into [`GLYPHS`]. The render crate rasterizes every
//! entry of this table into its atlas, so adding a glyph here is all it takes to
//! use it. Families are ordered light to heavy.

/// Every glyph the generator may place. Index 0 is the empty cell.
pub const GLYPHS: [char; 26] = [
    ' ', '\'', '-', '/', '|', '\\', '.', '=', '<', '>', ':', 'o', 'x', 'O', '#', 'X', '%', '@',
    '+', '^', '~', '`', ',', '*', ';', '"',
];

/// The empty cell.
pub const EMPTY: u8 = 0;

/// Index of `c` in [`GLYPHS`]. Panics at compile time if it is missing.
pub const fn id(c: char) -> u8 {
    let mut i = 0;
    while i < GLYPHS.len() {
        if GLYPHS[i] == c {
            return i as u8;
        }
        i += 1;
    }
    panic!("glyph missing from GLYPHS")
}

/// Character for a glyph id.
pub fn char_of(glyph: u8) -> char {
    GLYPHS[glyph as usize]
}

/// Far cliff hatch, left side: `'-/|`.
pub const FAR_LEFT: [u8; 4] = [id('\''), id('-'), id('/'), id('|')];
/// Far cliff hatch, right side (mirrored): `'-\|`.
pub const FAR_RIGHT: [u8; 4] = [id('\''), id('-'), id('\\'), id('|')];
/// Mid cliff hatch, left side: `.-=\<`.
pub const MID_LEFT: [u8; 5] = [id('.'), id('-'), id('='), id('\\'), id('<')];
/// Mid cliff hatch, right side: `.-=/>`.
pub const MID_RIGHT: [u8; 5] = [id('.'), id('-'), id('='), id('/'), id('>')];
/// Near, dense rock: `.:ox=O#X%@`.
pub const NEAR: [u8; 10] = [
    id('.'),
    id(':'),
    id('o'),
    id('x'),
    id('='),
    id('O'),
    id('#'),
    id('X'),
    id('%'),
    id('@'),
];
/// Platform, heavy: `=o+x#X%@`.
pub const PLATFORM: [u8; 8] = [
    id('='),
    id('o'),
    id('+'),
    id('x'),
    id('#'),
    id('X'),
    id('%'),
    id('@'),
];
/// Platform top edge.
pub const PLATFORM_TOP: u8 = id('^');
/// Daytime sky dust.
pub const SKY_DUST: [u8; 4] = [id('.'), id('\''), id('`'), id(',')];
/// Night sky stars, faint to bright.
pub const STARS: [u8; 6] = [id('.'), id('`'), id('\''), id(','), id(':'), id('*')];
/// Ember glyphs by age: hot sparks first, fading dots last.
pub const EMBERS: [u8; 5] = [id('*'), id('+'), id('\''), id('.'), id('`')];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_no_duplicates() {
        for (i, a) in GLYPHS.iter().enumerate() {
            for b in &GLYPHS[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(char_of(EMPTY), ' ');
    }
}
