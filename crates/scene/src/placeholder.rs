//! Milestone M1 placeholder pixel maps, taken from the Python prototype
//!. Milestone M2 replaces them with project-drawn sprite
//! sheets behind a `Character` trait in a `sprites` crate.
//!
//! `CLAWD_PLACEHOLDER` depicts Clawd, Anthropic's character. It is fan art and is
//! NOT covered by this crate's MIT/Apache-2.0 licence; see the NOTICE file. To
//! remove it (docs/dev/takedown.md), delete the constant and render with Buddy off.

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

const SLATE: Rgb = Rgb::hex(0x5E6B7A);
const SLATE_DARK: Rgb = Rgb::hex(0x3B4450);
const BRASS: Rgb = Rgb::hex(0xC9A45C);
const METAL: Rgb = Rgb::hex(0x3A3330);

/// A small telescope on a tripod (project art, CC BY 4.0).
pub const TELESCOPE: PixelMap = PixelMap {
    width: 12,
    height: 10,
    rows: &[
        ".........NN.",
        ".......TTNN.",
        ".....TTTT...",
        "...TTTT.....",
        "..TTT.......",
        "....H.......",
        "...L.L......",
        "..L...L.....",
        ".L.....L....",
        "L.......L...",
    ],
    colors: &[('T', SLATE), ('N', SLATE_DARK), ('H', BRASS), ('L', LOG)],
};

/// A hand lantern (project art, CC BY 4.0).
pub const LANTERN: PixelMap = PixelMap {
    width: 5,
    height: 7,
    rows: &[
        "..M..", ".M.M.", "MMMMM", "MYWYM", "MYWYM", "MYYYM", "MMMMM",
    ],
    colors: &[('M', METAL), ('Y', FIRE_YELLOW), ('W', FIRE_WHITE)],
};

const SPARK: Rgb = Rgb::hex(0xFFE9A0);
const PAGE: Rgb = Rgb::hex(0xEEEAE0);
const PAGE_LINE: Rgb = Rgb::hex(0xC8C2B0);
const COVER: Rgb = Rgb::hex(0x7A2A4A);
const WOOD: Rgb = Rgb::hex(0x7A5234);
const WOOD_DARK: Rgb = Rgb::hex(0x4E3422);
const STEEL: Rgb = Rgb::hex(0x5E6B7A);
const STEEL_DARK: Rgb = Rgb::hex(0x3B4450);
const HAT: Rgb = Rgb::hex(0x5B4B8A);
const HAT_DARK: Rgb = Rgb::hex(0x413566);

/// An open spellbook on a little stand, runes glowing (project art, CC BY 4.0).
pub const SPELLBOOK: PixelMap = PixelMap {
    width: 11,
    height: 9,
    rows: &[
        "..s.....s..",
        "....s.s....",
        "PPPPP.PPPPP",
        "PlllPBPlllP",
        "PlllPBPlllP",
        "RRRRRBRRRRR",
        "....WWW....",
        "....WWW....",
        "...WWWWW...",
    ],
    colors: &[
        ('s', SPARK),
        ('P', PAGE),
        ('l', PAGE_LINE),
        ('B', WOOD_DARK),
        ('R', COVER),
        ('W', WOOD),
    ],
};

/// A camp stove with a kettle on the flame (project art, CC BY 4.0).
pub const CAMP_STOVE: PixelMap = PixelMap {
    width: 9,
    height: 8,
    rows: &[
        "...MM....",
        "..M..M...",
        ".KKKKKKS.",
        ".KKKKKK.S",
        ".KKKKKK..",
        "OYOYOYO..",
        "GGGGGGG..",
        "G.G.G.G..",
    ],
    colors: &[
        ('M', STEEL_DARK),
        ('K', STEEL),
        ('S', STEEL),
        ('O', FIRE_ORANGE),
        ('Y', FIRE_YELLOW),
        ('G', METAL),
    ],
};

/// A coffee mug left on the ground: the unattended variant (project art, CC BY 4.0).
pub const COFFEE_MUG: PixelMap = PixelMap {
    width: 5,
    height: 4,
    rows: &["WCCW.", "WWWWW", "WWWW.", "WWWW."],
    colors: &[('W', PAGE), ('C', WOOD_DARK)],
};

/// A wizard hat and wand left on the ground: the unattended variant (project art, CC BY 4.0).
pub const FALLEN_HAT: PixelMap = PixelMap {
    width: 12,
    height: 6,
    rows: &[
        "......PP....",
        "....PPPPP...",
        "..PPPPPPPPS.",
        "DPPPPPPPPPP.",
        "DDDDDDDDDDDD",
        "www.wwwwwww.",
    ],
    colors: &[('P', HAT), ('D', HAT_DARK), ('S', SPARK), ('w', WOOD)],
};

const INK_DARK: Rgb = Rgb::hex(0x2A2624);

/// A closed book on the ground: the unattended variant of reading (project art, CC BY 4.0).
pub const BOOK_CLOSED: PixelMap = PixelMap {
    width: 7,
    height: 3,
    rows: &["RRRRRRR", "PPPPPPR", "RRRRRRR"],
    colors: &[('R', COVER), ('P', PAGE)],
};

/// A guitar leaning upright: the unattended variant of playing (project art, CC BY 4.0).
pub const GUITAR_REST: PixelMap = PixelMap {
    width: 7,
    height: 12,
    rows: &[
        "...KK..", "...d...", "...d...", "...d...", "...d...", "..WWW..", ".WWWWW.", ".WWKWW.",
        "..WWW..", ".WWWWW.", ".WWWWW.", "..WWW..",
    ],
    colors: &[('K', INK_DARK), ('d', WOOD_DARK), ('W', WOOD)],
};

/// A tin bucket with a wire handle: the unattended variant of fishing
/// (project art, CC BY 4.0).
pub const BUCKET: PixelMap = PixelMap {
    width: 7,
    height: 7,
    rows: &[
        "..KKK..", ".K...K.", "SSSSSSS", ".sssss.", ".SSSSS.", ".sssss.", "..SSS..",
    ],
    colors: &[('K', INK_DARK), ('S', STEEL), ('s', STEEL_DARK)],
};

const KITE_RED: Rgb = Rgb::hex(0xC8452E);
const KITE_WHITE: Rgb = Rgb::hex(0xEEEAE0);

/// A kite propped up on its spool: the unattended variant of kite flying
/// (project art, CC BY 4.0).
pub const KITE_REST: PixelMap = PixelMap {
    width: 5,
    height: 7,
    rows: &[
        "..R..", ".RRR.", "RRWRR", ".RRR.", "..R..", "..K..", ".dKd.",
    ],
    colors: &[
        ('R', KITE_RED),
        ('W', KITE_WHITE),
        ('K', INK_DARK),
        ('d', WOOD_DARK),
    ],
};

/// A few flat stones in a pile: the unattended variant of skipping stones
/// (project art, CC BY 4.0).
pub const STONE_PILE: PixelMap = PixelMap {
    width: 7,
    height: 3,
    rows: &["..sS...", ".SSsSs.", "sSSSsSS"],
    colors: &[('S', STEEL), ('s', STEEL_DARK)],
};

const PAINT_PURPLE: Rgb = Rgb::hex(0x5B4B8A);
const SNOW_SHADE: Rgb = Rgb::hex(0xC8C2B0);

/// An easel with a painting under way: the set piece for painting (project
/// art, CC BY 4.0).
pub const EASEL: PixelMap = PixelMap {
    width: 9,
    height: 11,
    rows: &[
        "....d....",
        ".PPPPPPP.",
        ".PRRPPYP.",
        ".PPRPYYP.",
        ".PUUPPPP.",
        ".PPPPUPP.",
        ".PPPPPPP.",
        "ddddddddd",
        "..d.d.d..",
        ".d..d..d.",
        "d...d...d",
    ],
    colors: &[
        ('d', WOOD_DARK),
        ('P', PAGE),
        ('R', KITE_RED),
        ('Y', BRASS),
        ('U', PAINT_PURPLE),
    ],
};

/// A snowman with a coal face and a hat: the target for snowballs (project
/// art, CC BY 4.0).
pub const SNOWMAN: PixelMap = PixelMap {
    width: 9,
    height: 14,
    rows: &[
        "...KKK...",
        "...KKK...",
        "..KKKKK..",
        "...WWW...",
        "..WKWKW..",
        "..WWRRW..",
        "...WWW...",
        "..WWWWW..",
        ".WWWKWWW.",
        ".WWWWWWS.",
        "WWWWKWWWS",
        "WWWWWWWWS",
        ".WWWWWWS.",
        "..WWWWS..",
    ],
    colors: &[
        ('K', INK_DARK),
        ('W', PAGE),
        ('S', SNOW_SHADE),
        ('R', KITE_RED),
    ],
};

/// An empty wicker basket on the ground: the unattended variant of the
/// balloon ride (project art, CC BY 4.0).
pub const BASKET: PixelMap = PixelMap {
    width: 9,
    height: 5,
    rows: &[
        "d.......d",
        "WWWWWWWWW",
        "WdWdWdWdW",
        "WWWWWWWWW",
        ".WdWdWdW.",
    ],
    colors: &[('W', WOOD), ('d', WOOD_DARK)],
};

/// A picnic basket with a checked cloth peeking out: the set piece for a
/// picnic (project art, CC BY 4.0).
pub const PICNIC_BASKET: PixelMap = PixelMap {
    width: 9,
    height: 8,
    rows: &[
        "...ddd...",
        "..d...d..",
        ".d.....d.",
        "RPRPRPRPR",
        "WWWWWWWWW",
        "WdWdWdWdW",
        "WWWWWWWWW",
        ".WdWdWdW.",
    ],
    colors: &[('d', WOOD_DARK), ('W', WOOD), ('R', KITE_RED), ('P', PAGE)],
};

/// A low table with a chessboard and two pieces: the set piece for chess
/// (project art, CC BY 4.0).
pub const CHESS_TABLE: PixelMap = PixelMap {
    width: 11,
    height: 9,
    rows: &[
        "...K...P...",
        "..KKK.PPP..",
        "KPKPKPKPKPK",
        "PKPKPKPKPKP",
        "ddddddddddd",
        ".d.......d.",
        ".d.......d.",
        ".d.......d.",
        ".d.......d.",
    ],
    colors: &[('K', INK_DARK), ('P', PAGE), ('d', WOOD_DARK)],
};

const TENNIS_BALL: Rgb = Rgb::hex(0xD8E04A);

/// A tennis practice wall on posts, with a painted target: the set piece for
/// tennis (project art, CC BY 4.0).
pub const PRACTICE_WALL: PixelMap = PixelMap {
    width: 7,
    height: 16,
    rows: &[
        "SSSSSSs", "SSSSSSs", "SSSBSSs", "SSBBBSs", "SSSBSSs", "SSSSSSs", "PPPPPPP", "SSSSSSs",
        "SSSSSSs", "SSSSSSs", "SSSSSSs", "SSSSSSs", ".d...d.", ".d...d.", ".d...d.", ".d...d.",
    ],
    colors: &[
        ('S', STEEL),
        ('s', STEEL_DARK),
        ('P', PAGE),
        ('B', TENNIS_BALL),
        ('d', WOOD_DARK),
    ],
};

/// A pair of skis and poles standing in the snow: the unattended variant of
/// skiing (project art, CC BY 4.0).
pub const SKIS_REST: PixelMap = PixelMap {
    width: 7,
    height: 14,
    rows: &[
        ".R.R...", ".R.R.s.", ".R.R.s.", ".R.R.s.", ".R.R.s.", ".R.R.s.", ".R.R.s.", ".R.R.s.",
        ".R.R.s.", ".R.R.s.", ".R.R.s.", ".R.R.s.", ".R.R.K.", "KKKKKKK",
    ],
    colors: &[('R', KITE_RED), ('s', STEEL_DARK), ('K', INK_DARK)],
};

const LEAF: Rgb = Rgb::hex(0x5E7A45);

/// Two flower pots, red blooms and white daisies: the set piece for
/// gardening (project art, CC BY 4.0).
pub const FLOWER_POTS: PixelMap = PixelMap {
    width: 11,
    height: 9,
    rows: &[
        ".R.R...P.P.",
        "..Y.....Y..",
        ".R.R...P.P.",
        "..G.....G..",
        ".GG.....GG.",
        "..G.....G..",
        "TTTTT.TTTTT",
        ".TTT...TTT.",
        ".ttt...ttt.",
    ],
    colors: &[
        ('R', KITE_RED),
        ('P', PAGE),
        ('Y', SPARK),
        ('G', LEAF),
        ('T', LOG),
        ('t', LOG_DARK),
    ],
};

/// A music stand with a page of music: the set piece for the flute
/// (project art, CC BY 4.0).
pub const MUSIC_STAND: PixelMap = PixelMap {
    width: 7,
    height: 12,
    rows: &[
        "PPPPPPP", "PLLLLLP", "PPKPPKP", "PLLLLLP", "PKPPKPP", "sssssss", "...s...", "...s...",
        "...s...", "...s...", "..s.s..", ".s...s.",
    ],
    colors: &[
        ('P', PAGE),
        ('L', PAGE_LINE),
        ('K', INK_DARK),
        ('s', STEEL_DARK),
    ],
};

/// A camera on a tripod: the unattended variant of photography (project
/// art, CC BY 4.0).
pub const CAMERA_TRIPOD: PixelMap = PixelMap {
    width: 7,
    height: 12,
    rows: &[
        "..KK...", ".KKKKK.", ".KKPKK.", ".KKKKK.", "...s...", "...s...", "..s.s..", "..s.s..",
        ".s...s.", ".s...s.", "s.....s", "s.....s",
    ],
    colors: &[('K', INK_DARK), ('P', PAGE), ('s', STEEL_DARK)],
};

/// A sandcastle with a flag on its keep: the set piece for building one
/// (project art, CC BY 4.0).
pub const SANDCASTLE: PixelMap = PixelMap {
    width: 11,
    height: 10,
    rows: &[
        "....KR.....",
        "....KRR....",
        "....K......",
        "...B.B.B...",
        "...BBBBB...",
        "B.B.BBB.B.B",
        "BBBBBBBBBBB",
        "BBBBdddBBBB",
        "BBBBdddBBBB",
        "WWWWWWWWWWW",
    ],
    colors: &[
        ('K', INK_DARK),
        ('R', KITE_RED),
        ('B', BRASS),
        ('d', WOOD_DARK),
        ('W', WOOD),
    ],
};

/// A rolled yoga mat: the set piece for yoga (project art, CC BY 4.0).
pub const YOGA_MAT: PixelMap = PixelMap {
    width: 9,
    height: 4,
    rows: &[".HHHHHHH.", "hHHHHHHHh", "hHHHHHHHh", ".HHHHHHH."],
    colors: &[('H', HAT), ('h', HAT_DARK)],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_are_rectangular_and_fully_coloured() {
        for map in [
            CLAWD_PLACEHOLDER,
            CAMPFIRE,
            LOG_SEAT,
            TELESCOPE,
            LANTERN,
            SPELLBOOK,
            CAMP_STOVE,
            COFFEE_MUG,
            FALLEN_HAT,
            BOOK_CLOSED,
            GUITAR_REST,
            BUCKET,
            KITE_REST,
            STONE_PILE,
            EASEL,
            SNOWMAN,
            BASKET,
            PICNIC_BASKET,
            CHESS_TABLE,
            PRACTICE_WALL,
            SKIS_REST,
            FLOWER_POTS,
            MUSIC_STAND,
            CAMERA_TRIPOD,
            SANDCASTLE,
            YOGA_MAT,
        ] {
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
