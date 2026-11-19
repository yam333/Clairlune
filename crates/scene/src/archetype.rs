//! Archetypes declare the capabilities they offer. Activities declare the
//! capabilities they need. Neither names the other, so a new archetype works with
//! existing activities and vice versa (PLAN.md section 4.7).

/// Terrain capabilities an archetype can offer.
pub const TERRAINS: [&str; 9] = [
    "flat",
    "ledge",
    "cliff",
    "slope",
    "dune",
    "shore",
    "rooftop",
    "cave_floor",
    "water",
];
/// Surface materials.
pub const SURFACES: [&str; 8] = [
    "rock", "snow", "sand", "grass", "wood", "roof", "water", "ice",
];
/// Ambient weather. Light-source effects such as embers belong to props, not weather.
pub const WEATHERS: [&str; 8] = [
    "none",
    "dust",
    "snow",
    "rain",
    "fireflies",
    "leaves",
    "stars",
    "confetti",
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

/// The nine v1 archetypes (PLAN.md section 4.2). Unimplemented ones carry their
/// intended capabilities so activity files can be written against them now.
pub const ARCHETYPE_INFO: [ArchetypeInfo; 9] = [
    ArchetypeInfo {
        id: "canyon",
        implemented: true,
        terrain: &["ledge", "cliff"],
        surface: &["rock"],
        weather: &["none"],
        platforms: &["rock_ledge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "forest",
        implemented: true,
        terrain: &["flat", "ledge"],
        surface: &["grass", "wood"],
        weather: &["none", "fireflies", "leaves", "rain"],
        platforms: &["log", "tree_branch"],
        sky: true,
    },
    ArchetypeInfo {
        id: "mountains",
        implemented: true,
        terrain: &["ledge", "cliff", "slope"],
        surface: &["rock", "snow"],
        weather: &["none", "snow", "stars"],
        platforms: &["rock_ledge"],
        sky: true,
    },
    ArchetypeInfo {
        id: "shore",
        implemented: true,
        terrain: &["shore", "water", "flat"],
        surface: &["sand", "wood", "water"],
        weather: &["none", "rain", "stars"],
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
        terrain: &["dune", "slope", "flat"],
        surface: &["sand"],
        weather: &["none", "dust", "stars"],
        platforms: &["dune_crest"],
        sky: true,
    },
    ArchetypeInfo {
        id: "snowfield",
        implemented: true,
        terrain: &["slope", "flat", "ledge"],
        surface: &["snow", "ice"],
        weather: &["none", "snow", "stars"],
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
];

/// Looks up an archetype by id.
pub fn archetype(id: &str) -> Option<&'static ArchetypeInfo> {
    ARCHETYPE_INFO.iter().find(|a| a.id == id)
}
