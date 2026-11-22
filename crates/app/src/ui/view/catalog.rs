//! What scenes can be made of: the landscapes, activities, weather,
//! looks and zooms Settings lists.

/// Filter items: id, short label, and what limits it (shown on hover).
pub(crate) type Item = (&'static str, &'static str, &'static str);

/// The looks, by their ids in the recipe.
pub(crate) const LOOKS: [Item; 2] = [
    ("line", "Line art", "The land in strokes of text"),
    (
        "dither",
        "Dithered",
        "Light and dark masses of dots and bars",
    ),
];

/// The zooms, by their ids in the recipe.
pub(crate) const ZOOMS: [Item; 3] = [
    ("close", "Close", "Clawd up close, as he has always been"),
    (
        "wide",
        "Wide",
        "Clawd about half the size, in a bigger landscape",
    ),
    ("vast", "Vast", "Clawd tiny in a huge landscape"),
];

pub(crate) const LANDSCAPES: [Item; 27] = [
    ("canyon", "Canyon", ""),
    ("mountains", "Mountains", ""),
    ("forest", "Forest", ""),
    ("desert", "Desert", ""),
    ("snowfield", "Snowfield", ""),
    ("shore", "Shore", ""),
    ("city_rooftops", "Rooftops", "City rooftops"),
    ("night_sky", "Night sky", ""),
    ("cave", "Cave", ""),
    (
        "interior",
        "Indoors",
        "A room, with a landscape through the window",
    ),
    ("meadow", "Meadow", "Rolling grass and wildflowers"),
    ("lake", "Lake", "Mountains mirrored in still water"),
    ("volcano", "Volcano", "A smoking cone and lava fields"),
    ("islands", "Islands", "Palm islets on a calm sea"),
    ("lunar", "The Moon", "Craters, with the Earth in the sky"),
    ("bamboo", "Bamboo", "A bamboo grove"),
    ("waterfall", "Waterfall", "A fall into a misty pool"),
    (
        "countryside",
        "Countryside",
        "Fields, a windmill and a barn",
    ),
    ("ruins", "Ruins", "Old columns and an arch"),
    ("savanna", "Savanna", "Golden grass and acacias"),
    ("swamp", "Swamp", "Cypress trees in still water"),
    (
        "mushrooms",
        "Giant mushrooms",
        "A grove of towering toadstools",
    ),
    ("crystals", "Crystal cave", "Glowing crystals, no sky"),
    (
        "great_tree",
        "Great tree",
        "A colossal tree hung with lanterns",
    ),
    (
        "wizard_tower",
        "Wizard's tower",
        "A crooked tower on a crag",
    ),
    (
        "standing_stones",
        "Standing stones",
        "A ring of stones with glowing runes",
    ),
    (
        "lantern_lake",
        "Lantern lake",
        "Floating lanterns, a gate in the water",
    ),
];

pub(crate) const ACTIVITIES: [Item; 38] = [
    ("warm_hands", "Campfire", "Warming hands by a fire"),
    ("stroll", "Strolling", ""),
    ("stargaze", "Stargazing", "Night only"),
    ("wizard_magic", "Spells", "Casting spells"),
    ("make_coffee", "Coffee", "Making coffee"),
    ("read_lantern", "Reading", "Reading by a lantern"),
    ("play_guitar", "Guitar", "Playing guitar"),
    ("fishing", "Fishing", "Off a pier, shore only"),
    ("fly_kite", "Kite", "Flying a kite, daytime"),
    ("skip_stones", "Skipping stones", "Shore only"),
    ("broom_flight", "Broom flight", "Evening and night"),
    ("paint_easel", "Painting", "At an easel, daytime"),
    ("snowballs", "Snowballs", "Snow only"),
    ("balloon_ride", "Balloon ride", "A hot-air balloon, daytime"),
    ("picnic", "Picnic", "Daytime"),
    ("chess", "Chess", ""),
    ("tennis", "Tennis", "Against a wall, daytime"),
    ("rowing", "Rowing", "A boat, shore only"),
    ("skiing", "Skiing", "Snow only"),
    (
        "hike_ridge",
        "Ridge hike",
        "Mountains; the land scrolls past",
    ),
    ("ski_tour", "Ski touring", "Snow; the land scrolls past"),
    ("sledding", "Sledding", "Snow; the land scrolls past"),
    (
        "forest_walk",
        "Forest walk",
        "Forest; the land scrolls past",
    ),
    (
        "desert_trek",
        "Desert trek",
        "Desert; the land scrolls past",
    ),
    (
        "broom_journey",
        "Broom journey",
        "Evening and night; the land scrolls past below",
    ),
    (
        "balloon_journey",
        "Balloon journey",
        "Daytime; the land drifts past below",
    ),
    ("bubbles", "Bubbles", "Blowing bubbles, daytime"),
    ("meditate", "Meditating", "By a cairn"),
    ("dance", "Dancing", "Evening and night"),
    (
        "catch_snowflakes",
        "Snowflakes",
        "Catching them, when it snows",
    ),
    ("sweep", "Sweeping", "Daytime"),
    ("hot_cocoa", "Hot cocoa", "In the cold"),
    ("gardening", "Gardening", "Watering flowers, daytime"),
    ("play_flute", "Flute", "Playing the flute"),
    ("photography", "Photography", "Photos of the view"),
    ("birdwatching", "Birdwatching", "When birds are about"),
    ("sandcastle", "Sandcastle", "On sand, daytime"),
    ("yoga", "Yoga", "Stretches by a mat"),
];

/// Weather that shows as particles. Clear weather ("none", "stars") is
/// always allowed so every scene has something to fall back to.
pub(crate) const WEATHER: [Item; 15] = [
    ("snow", "Snow", ""),
    ("rain", "Rain", ""),
    ("fireflies", "Fireflies", ""),
    ("leaves", "Leaves", "Falling leaves"),
    ("dust", "Dust", ""),
    ("petals", "Petals", "Blossom drifting down"),
    ("fog", "Fog", "Mist along the ground"),
    ("blizzard", "Blizzard", "Snow on a gale"),
    ("ash", "Ash", "Falling ash, by a volcano"),
    ("sparkle", "Frost", "Ice crystals glinting in the cold"),
    ("confetti", "Confetti", "Over the rooftops"),
    ("birds", "Birds", "Crossing the sky by day"),
    ("butterflies", "Butterflies", "Fluttering low, by day"),
    ("seeds", "Dandelion seeds", "Drifting on the breeze"),
    (
        "shooting_stars",
        "Shooting stars",
        "Streaking by after dark",
    ),
];
pub(crate) const CLEAR_WEATHER: [&str; 2] = ["none", "stars"];
