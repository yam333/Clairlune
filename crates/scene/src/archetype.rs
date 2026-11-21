//! Archetypes declare the capabilities they offer. Activities declare the
//! capabilities they need. Neither names the other, so a new archetype works with
//! existing activities and vice versa (PLAN.md section 4.7).

/// Terrain capabilities an archetype can offer.
pub const TERRAINS: [&str; 14] = [
    "flat",
    "ledge",
    "cliff",
    "slope",
    "dune",
    "shore",
    "rooftop",
    "cave_floor",
    "water",
    "indoor",
    "ridge",
    "trail",
    "path",
    "dune_trail",
];
/// Surface materials.
pub const SURFACES: [&str; 8] = [
    "rock", "snow", "sand", "grass", "wood", "roof", "water", "ice",
];
/// Ambient weather. Light-source effects such as embers belong to props, not weather.
pub const WEATHERS: [&str; 13] = [
    "none",
    "dust",
    "snow",
    "rain",
    "fireflies",
    "leaves",
    "stars",
    "confetti",
    "petals",
    "fog",
    "blizzard",
    "ash",
    "sparkle",
];

/// What one archetype offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchetypeInfo {
    pub id: &'static str,
    /// Whether this generator version can build it.
    pub implemented: bool,
    pub terrain: &'static [&'static str],
    pub surface: &'static [&'static str],
    pub weather: &'static [&'static str],
    pub platforms: &'static [&'static str],
    /// Open sky overhead (stars, moon). False for enclosed places such as caves.
    pub sky: bool,
}

/// The archetypes (PLAN.md section 4.2, and the interior from
/// docs/activity-system.md). Unimplemented ones carry their
/// intended capabilities so activity files can be written against them now.
pub const ARCHETYPE_INFO: [ArchetypeInfo; 16] = [
    ArchetypeInfo {
        id: "canyon",
        implemented: true,
        terrain: &["ledge", "cliff"],
        surface: &["rock"],
        weather: &["none", "dust"],
        platforms: &["rock_ledge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "forest",
        implemented: true,
        // `path`: a periodic variant for walks through the wood.
        terrain: &["flat", "ledge", "path"],
        surface: &["grass", "wood"],
        weather: &["none", "fireflies", "leaves", "rain", "fog", "petals"],
        platforms: &["log", "tree_branch"],
        sky: true,
    },
    ArchetypeInfo {
        id: "mountains",
        implemented: true,
        // `ridge`: a periodic variant for journeys along the range.
        terrain: &["ledge", "cliff", "slope", "ridge"],
        surface: &["rock", "snow"],
        weather: &["none", "snow", "stars", "fog", "blizzard"],
        platforms: &["rock_ledge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "shore",
        implemented: true,
        terrain: &["shore", "water", "flat"],
        surface: &["sand", "wood", "water"],
        weather: &["none", "rain", "stars", "fog"],
        platforms: &["pier"],
        sky: true,
    },
    ArchetypeInfo {
        id: "city_rooftops",
        implemented: true,
        terrain: &["rooftop", "flat"],
        surface: &["roof"],
        weather: &["none", "rain", "stars", "confetti"],
        platforms: &["rooftop_edge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "desert",
        implemented: true,
        // `dune_trail`: a periodic variant for journeys over the dunes.
        terrain: &["dune", "slope", "flat", "dune_trail"],
        surface: &["sand"],
        weather: &["none", "dust", "stars"],
        platforms: &["dune_crest"],
        sky: true,
    },
    ArchetypeInfo {
        id: "snowfield",
        implemented: true,
        // `trail`: a periodic variant for journeys across the snow.
        terrain: &["slope", "flat", "ledge", "trail"],
        surface: &["snow", "ice"],
        weather: &["none", "snow", "stars", "blizzard", "sparkle"],
        platforms: &["ice_shelf"],
        sky: true,
    },
    ArchetypeInfo {
        id: "night_sky",
        implemented: true,
        terrain: &["ledge", "flat"],
        surface: &["rock", "grass"],
        weather: &["none", "stars"],
        platforms: &["floating_island", "rock_ledge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "cave",
        implemented: true,
        terrain: &["cave_floor", "ledge"],
        surface: &["rock"],
        weather: &["none", "dust"],
        platforms: &["rock_ledge"],
        sky: false,
    },
    ArchetypeInfo {
        id: "meadow",
        implemented: true,
        terrain: &["flat", "ledge"],
        surface: &["grass"],
        weather: &[
            "none",
            "petals",
            "leaves",
            "fireflies",
            "rain",
            "fog",
            "stars",
        ],
        platforms: &["grassy_knoll"],
        sky: true,
    },
    ArchetypeInfo {
        id: "lake",
        implemented: true,
        terrain: &["flat", "ledge"],
        surface: &["grass", "rock"],
        weather: &["none", "fog", "rain", "fireflies", "snow", "stars"],
        platforms: &["grassy_bank"],
        sky: true,
    },
    ArchetypeInfo {
        id: "volcano",
        implemented: true,
        terrain: &["ledge", "flat"],
        surface: &["rock"],
        weather: &["none", "ash", "stars"],
        platforms: &["rock_ledge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "islands",
        implemented: true,
        terrain: &["flat"],
        surface: &["sand"],
        weather: &["none", "rain", "stars"],
        platforms: &["sand_spit"],
        sky: true,
    },
    ArchetypeInfo {
        id: "lunar",
        implemented: true,
        terrain: &["flat", "ledge"],
        surface: &["rock"],
        weather: &["none", "stars"],
        platforms: &["crater_rim"],
        sky: true,
    },
    ArchetypeInfo {
        id: "bamboo",
        implemented: true,
        terrain: &["flat"],
        surface: &["grass"],
        weather: &["none", "rain", "fog", "fireflies", "leaves"],
        platforms: &["mossy_bank"],
        sky: false,
    },
    ArchetypeInfo {
        id: "interior",
        implemented: true,
        terrain: &["indoor"],
        surface: &["wood"],
        weather: &["none"],
        platforms: &["floor"],
        sky: false,
    },
];

/// Looks up an archetype by id.
pub fn archetype(id: &str) -> Option<&'static ArchetypeInfo> {
    ARCHETYPE_INFO.iter().find(|a| a.id == id)
}
