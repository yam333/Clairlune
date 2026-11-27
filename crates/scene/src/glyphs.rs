//! The global glyph table and the glyph families used by layers.
//!
//! A cell stores a `u8` index into [`GLYPHS`]. The render crate rasterizes every
//! entry of this table into its atlas, so adding a glyph here is all it takes to
//! use it. Families are ordered light to heavy.

/// Every glyph the generator may place. Index 0 is the empty cell. New
/// glyphs go at the end, so saved scenes keep their ids.
pub const GLYPHS: [char; 38] = [
    ' ', '\'', '-', '/', '|', '\\', '.', '=', '<', '>', ':', 'o', 'x', 'O', '#', 'X', '%', '@',
    '+', '^', '~', '`', ',', '*', ';', '"', 'd', 'J', '_', 'v', '!', '&', // the line art
    '·', '•', '▪', '━', '●', '■', // the dither ramp
];

/// The empty cell.
pub const EMPTY: u8 = 0;

/// The letter sets a scene can be drawn in: the glyphs as they are (`text`),
/// or each one swapped for a character of a like shape and weight. The swap
/// happens when glyphs are rasterized, so everything that draws glyphs
/// (land, sky, weather, embers) takes the set.
pub const LETTERS: [&str; 7] = [
    "text", "braille", "blocks", "lines", "kana", "shapes", "hex",
];

/// The characters glyph ids stand for in a letter set (unknown: `text`).
pub fn letters(set: &str) -> [char; GLYPHS.len()] {
    match set {
        "braille" => BRAILLE,
        "blocks" => BLOCKS,
        "lines" => LINES,
        "kana" => KANA,
        "shapes" => SHAPES,
        "hex" => HEX,
        _ => GLYPHS,
    }
}

/// Braille: each glyph's shape at two by four dots.
const BRAILLE: [char; GLYPHS.len()] = [
    ' ', '⠈', '⠒', '⡜', '⡇', '⢣', '⡀', '⠶', '⠪', '⠕', '⠅', '⠶', '⠭', '⠿', '⡿', '⣟', '⣯', '⣿', '⠺',
    '⠊', '⠢', '⠁', '⢀', '⠮', '⠆', '⠉', '⢾', '⠼', '⣀', '⠣', '⠇', '⣾', '⠂', '⠆', '⠶', '⠿', '⡿', '⣿',
];

/// Blocks: quarter blocks for strokes, shades for masses.
const BLOCKS: [char; GLYPHS.len()] = [
    ' ', '▝', '▀', '▞', '▌', '▚', '▖', '░', '▙', '▟', '░', '▒', '▒', '▓', '▓', '▓', '▓', '█', '▒',
    '▀', '░', '▘', '▗', '▪', '▗', '▀', '▙', '▟', '▄', '▞', '▐', '▓', '▖', '░', '▒', '▀', '▓', '█',
];

/// Lines: box-drawing strokes and crossings.
const LINES: [char; GLYPHS.len()] = [
    ' ', '╵', '─', '╱', '│', '╲', '·', '═', '┤', '├', '┊', '┼', '╳', '╋', '╬', '╳', '╪', '╬', '┼',
    '┴', '∼', '╵', '╷', '┼', '┆', '╵', 'd', 'J', '─', '∨', '╎', '╬', '╴', '─', '┼', '━', '╋', '█',
];

/// Kana: katakana of a like weight and shape (drawn narrowed to the cell).
const KANA: [char; GLYPHS.len()] = [
    ' ', 'ャ', 'ー', 'ノ', 'リ', 'ヽ', '・', 'ニ', 'ク', 'シ', 'ミ', 'ロ', 'メ', 'モ', 'ヰ', 'ヌ',
    'ム', 'ヲ', 'ナ', 'ヘ', 'ン', 'ッ', 'ュ', 'キ', 'シ', 'ハ', 'd', 'J', 'ー', 'ソ', 'イ', 'ツ',
    '・', 'ー', 'ニ', 'テ', 'モ', 'ヰ',
];

/// Shapes: geometric circles, squares, triangles and diamonds by weight.
const SHAPES: [char; GLYPHS.len()] = [
    ' ', '◦', '─', '╱', '│', '╲', '·', '≡', '◀', '▶', '•', '○', '×', '◇', '■', '◆', '●', '■', '+',
    '▲', '≈', '·', '·', '◇', '•', '▫', 'd', 'J', '─', '▼', '│', '●', '·', '◦', '▫', '▪', '●', '■',
];

/// Hex: hexadecimal digits (`0`..`F`) matched by stroke and density.
const HEX: [char; GLYPHS.len()] = [
    ' ', '1', '7', '7', '1', 'A', '0', 'E', 'C', 'D', '1', '0', 'A', '0', 'F', 'B', '8', '8', '4',
    'A', '2', '1', 'C', 'F', '3', '7', 'D', '9', '5', 'C', '1', '6', '1', '7', 'C', 'E', 'B', '8',
];

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
/// Distant haze: far mountains, far dunes.
pub const DISTANT: [u8; 4] = [id('`'), id('.'), id('\''), id(':')];
/// Ridges and hills.
pub const RIDGE: [u8; 6] = [id('.'), id(':'), id('-'), id('='), id('+'), id('x')];
/// Snow surfaces, sparse.
pub const SNOW: [u8; 4] = [id('.'), id('\''), id('`'), id(',')];
/// Sand: dunes with horizontal ripples.
pub const SAND: [u8; 5] = [id('.'), id(','), id(':'), id('-'), id('~')];
/// Foliage: tree canopies, bushes.
pub const FOLIAGE: [u8; 5] = [id('.'), id('o'), id('&'), id('%'), id('@')];
/// Ice shelf body.
pub const ICE: [u8; 4] = [id('-'), id('='), id('+'), id('#')];
/// Grass and earth: forest floor.
pub const EARTH: [u8; 5] = [id('.'), id(','), id(':'), id('o'), id('%')];

/// Music notes (the font has no \u{266A}; `d` and `J` read as notes).
pub const NOTES: [u8; 2] = [id('d'), id('J')];
/// The dithered look's ramp, by how much of the cell each glyph inks (in
/// the bundled font: 0, 2.7, 5.8, 11, 16, 39 and 46 %). The bar and the
/// square span the whole cell width, so bright rows join into lines.
pub const DITHER: [u8; 7] = [EMPTY, id('·'), id('•'), id('▪'), id('━'), id('●'), id('■')];

/// Ember glyphs by age: hot sparks first, fading dots last.
pub const EMBERS: [u8; 5] = [id('*'), id('+'), id('\''), id('.'), id('`')];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_letter_set_keeps_the_empty_cell_and_draws_the_rest() {
        for set in LETTERS {
            let t = letters(set);
            assert_eq!(t[EMPTY as usize], ' ', "{set}");
            assert!(t[1..].iter().all(|c| *c != ' '), "{set}");
        }
        assert_eq!(letters("text"), GLYPHS);
        assert_eq!(letters("unknown"), GLYPHS);
    }

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
