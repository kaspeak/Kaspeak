use kaspa_consensus_core::network::{NetworkId, NetworkType};
use opus::Channels;

/// Список прилагательных для генерации имен пользователей. Их длина не превышает 7 символов.
#[rustfmt::skip]
pub const ADJECTIVES: [&str; 205] = [
    "Able", "Agile", "Alert", "Angry", "Ashen", "Basic", "Bright", "Calm", "Chilly", "Clean",
    "Clever", "Cloudy", "Cozy", "Crazy", "Crisp", "Cruel", "Cuddly", "Cute", "Dark", "Daring",
    "Decent", "Deep", "Dense", "Dirty", "Dry", "Eager", "Early", "Earthy", "Edgy", "Fair", "Fancy",
    "Fatal", "Fierce", "Final", "Fine", "Flashy", "Fresh", "Frigid", "Frosty", "Funny", "Gentle",
    "Giant", "Gloomy", "Grace", "Grand", "Grave", "Green", "Grumpy", "Happy", "Hardy", "Harsh",
    "Hasty", "Heavy", "Hilly", "Icy", "Jolly", "Juicy", "Keen", "Kind", "Large", "Late", "Light",
    "Lively", "Lofty", "Lonely", "Loose", "Lucky", "Lush", "Mad", "Meek", "Messy", "Mild", "Misty",
    "Moody", "Narrow", "Neat", "Nifty", "Noisy", "Odd", "Pale", "Plain", "Plush", "Posh", "Proud",
    "Quick", "Quiet", "Rapid", "Rare", "Raw", "Ready", "Red", "Rich", "Rough", "Round", "Royal",
    "Sad", "Safe", "Salty", "Sane", "Sharp", "Shiny", "Short", "Shy", "Silent", "Simple", "Slim",
    "Smart", "Smooth", "Soft", "Sole", "Solid", "Sour", "Spicy", "Stale", "Stark", "Steady",
    "Stern", "Sticky", "Stormy", "Strong", "Sweet", "Swift", "Tangy", "Tasty", "Tiny", "Tough",
    "Tricky", "True", "Vague", "Vast", "Vivid", "Warm", "Weak", "Weary", "Wet", "White", "Wide",
    "Wild", "Wise", "Witty", "Wooden", "Young", "Zany", "Zealous", "Zesty", "Zippy", "Brave",
    "Cheer", "Crispy", "Dapper", "Elder", "Elegant", "Feisty", "Fuzzy", "Glassy", "Gleeful",
    "Humble", "Joyful", "Kooky", "Lovely", "Mellow", "Merry", "Mighty", "Modest", "Nasty",
    "Nimble", "Nutty", "Polite", "Quaint", "Quirky", "Rustic", "Savvy", "Sincere", "Silky",
    "Sleek", "Sloppy", "Snug", "Spiky", "Spongy", "Spry", "Stark", "Sturdy", "Subtle", "Sunny",
    "Tame", "Tense", "Timid", "Tired", "Tricky", "Unique", "Vivid", "Wacky", "Wealthy", "Wicked",
    "Wily", "Windy", "Winsome", "Witty", "Wry", "Yearly", "Yellow", "Yummy", "Zonal", "Zoned", "Rusty",
];

/// Список существительных для генерации имен пользователей. Их длина не превышает 7 символов.
#[rustfmt::skip]
pub const NOUNS: [&str; 299] = [
    "Apple", "Angel", "Arrow", "Beach", "Bear", "Berry", "Bird", "Blade", "Bridge", "Brush",
    "Candle", "Castle", "Chair", "Cloud", "Coin", "Crown", "Dance", "Daisy", "Dawn", "Dream",
    "Eagle", "Earth", "Flame", "Flower", "Forest", "Fruit", "Ghost", "Grace", "Grass", "Green",
    "Heart", "Hill", "Honey", "Horse", "House", "Jewel", "Joy", "Lake", "Leaf", "Lion", "Light",
    "Moon", "Music", "Night", "Ocean", "Peace", "Pearl", "River", "Rose", "Ruby", "Sand", "Sky",
    "Snow", "Star", "Stone", "Storm", "Sun", "Swan", "Tree", "Truth", "Wind", "World", "Youth",
    "Zebra", "Zone", "Armor", "Army", "Artist", "Atlas", "Atom", "Badge", "Band", "Bank", "Barrel",
    "Basket", "Beast", "Bed", "Bee", "Bell", "Belt", "Bench", "Block", "Blood", "Board", "Boat",
    "Body", "Bone", "Book", "Boot", "Bottle", "Box", "Boy", "Brand", "Bread", "Brick", "Broth",
    "Brush", "Bucket", "Bullet", "Button", "Cabin", "Cable", "Cake", "Camp", "Candy", "Cap", "Car",
    "Card", "Care", "Cargo", "Cart", "Case", "Cash", "Cat", "Cave", "Cell", "Chain", "Chalk",
    "Change", "Chart", "Check", "Chef", "Chest", "Child", "Chin", "City", "Class", "Clay", "Clock",
    "Cloth", "Club", "Coat", "Comb", "Comic", "Cone", "Cook", "Copy", "Corner", "Couch", "Court",
    "Cover", "Cow", "Craft", "Crate", "Cream", "Crew", "Crime", "Cruise", "Dog", "Door", "Dot",
    "Dragon", "Duck", "Dust", "Egg", "Engine", "Farm", "Feather", "Fence", "Fire", "Fish", "Flag",
    "Flute", "Fork", "Garden", "Gate", "Guitar", "Hammer", "Hat", "Helmet", "Hook", "Horn", "Ice",
    "Ink", "Iron", "Jacket", "Jeans", "Kettle", "Knife", "Lamp", "Leg", "Lemon", "Lock", "Maple",
    "Mask", "Match", "Mouth", "Neck", "Nose", "Page", "Paint", "Park", "Pear", "Pencil", "Pillow",
    "Plane", "Plate", "Point", "Pumpkin", "Rain", "Rock", "Roof", "Root", "Salt", "Scale",
    "School", "Shoe", "Shop", "Silver", "Sink", "Skate", "Snake", "Soap", "Sock", "Spoon",
    "Spring", "Store", "Sugar", "Swim", "Table", "Tank", "Tea", "Tent", "Thread", "Thumb",
    "Ticket", "Tiger", "Toe", "Tongue", "Tool", "Top", "Train", "Trip", "Truck", "Tunnel", "Vase",
    "Violin", "Wall", "Water", "Whale", "Window", "Wolf", "Wood", "Wool", "Yard", "Yogurt", "Zip",
    "Zoo", "Ace", "Bolt", "Cape", "Dove", "Echo", "Fawn", "Gaze", "Hawk", "Ivory", "Jade",
    "Knight", "Lace", "Mint", "Nest", "Oven", "Pine", "Quest", "Rune", "Spike", "Twig", "Vine",
    "Wheat", "Xylem", "Yarn", "Zeal", "Aura", "Blaze", "Crux", "Drift", "Ember", "Fable", "Glint",
    "Haze", "Igloo", "Jolt", "Keel", "Loom", "Moss", "Nook", "Oath", "Plume", "Quill", "Ridge",
    "Sleet", "Thorn", "Urn", "Veil", "Whisk", "Yacht", "Zinc", "Kaspa",
];

#[rustfmt::skip]
pub const EMOJIS: [&str; 70] = [
    "😀", "😂", "😎", "😍", "🥰", "😇", "😉", "😊", "😋", "😜",
    "🤪", "😝", "🤑", "🤗", "🤔", "🤨", "😐", "😑", "😶", "🙄",
    "😏", "😒", "😞", "😔", "😟", "😕", "🙁", "☹️", "😣", "😖",
    "😫", "😩", "🥺", "😭", "😢", "😤", "😠", "😡", "🤬", "🤯",
    "😳", "😱", "🥵", "🥶", "😰", "😥", "😓", "🤗", "🤭", "🧐",
    "🌚", "🐔", "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼",
    "🐻‍❄️", "🐨", "🐯", "🦁", "🐮", "🐷", "🐸", "🐵", "🦄", "🐙",
];

//SETTINGS
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SETTINGS_PATH: &'static str = "settings.kspk";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_LOGS_PATH: &'static str = "kaspeak.log";

#[cfg(target_os = "macos")]
pub const DEFAULT_SETTINGS_PATH: &'static str = "/Library/Caches/Kaspeak/settings.kspk";
#[cfg(target_os = "macos")]
pub const DEFAULT_LOGS_PATH: &'static str = "/Library/Caches/Kaspeak/logs/kaspeak.log";
//todo
pub const KSPK_ENCRYPTION_KEY: [u8; 32] = *b"E31CCF4FDF6446A2712294C6C757398F";

// PLAYER
pub const SAMPLE_RATE: u32 = 48000; // Частота дискретизации аудио
pub const CHANNELS: Channels = Channels::Mono;
pub const FRAME_SIZE: usize = (SAMPLE_RATE as f32 * FRAME_DURATION_MS as f32 / 1000.0) as usize;

// RECORDER
pub const FRAME_DURATION_MS: usize = 20; // Длительность одного аудиофрейма в миллисекундах
pub const OPUS_BITRATE: i32 = 32000; // Битрейт для Opus-энкодера
pub const OPUS_MAX_PACKET_SIZE: usize = 4000; // Максимальный размер пакета Opus

// РАЗМЕР 1 КАСПЫ В СОМПИ
pub const UNIT: f64 = 100_000_000.0;

// НАЧАЛЬНЫЙ РАЗМЕР КОМИССИИ
pub const DEFAULT_FEE_LEVEL: u64 = 1_000_000;

// НАЧАЛЬНЫЙ КАНАЛ
pub const DEFAULT_CHANNEL: u32 = 0;

// ОГРАНИЧЕНИЕ РАЗМЕРА КАНАЛА
pub const MAX_CHANNEL_CAPACITY: usize = 250;

// МИНИМАЛЬНЫЙ РАЗМЕР БАЛАНСА ДЛЯ НАЧАЛА ЭЙРДРОПА
pub const MINIMUM_AIRDROP_BALANCE_TKAS: f64 = 10.0f64;

// АЙДИ СЕТИ
pub const NETWORK_ID: NetworkId = NetworkId::with_suffix(NetworkType::Testnet, 11);

// ПРЕФИКС АДРЕСА
pub const PREFIX: &str = "kaspatest:";

pub static NOTIFICATION_SOUND_FILE_INLINED: &'static [u8] = include_bytes!("../assets/notification.wav");
pub static APP_ICON_FILE_INLINED: &'static [u8] = include_bytes!("../assets/256x256_1.png");

// PAYLOADS
pub const ZSTD_COMPRESSION_LEVEL: i32 = 3;
pub const MARKER: &[u8] = b"KSPK";
pub const PROTOCOL_VERSION: u8 = 0;

/// Полный размер «жёсткой» части заголовка (17 байт):
///   4 (MARKER) + 1 (VERSION) + 3 (CHANNEL) + 1 (MESSAGE_TYPE)
/// + 1 (STATUS_FLAG) + 3 (FRAGMENT) + 1 (USERNAME_LEN) + 3 (MESSAGE_SIZE)
pub const HEADER_SIZE: usize = 17;

pub const MAX_USERNAME_CHARS: usize = 18;
pub const MAX_USERNAME_BYTES: usize = 255;
pub const MAX_TEXT_CHARS: usize = 1000;
pub const MAX_PAYLOAD_BYTES: usize = 15_000;
