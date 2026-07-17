use std::fmt;

/// The corruption marker left behind where the source PDF's font is missing a
/// glyph (usually an "fi"/"fl"/"ff" ligature the SwimTopia PDF generator emits
/// but doesn't map back to text). We normalize every such gap to this one
/// character so it's easy to spot and easy to paste into a corrections file.
pub const CORRUPTION_MARKER: char = '\u{FFFD}';

#[derive(Debug, Clone)]
pub struct Meet {
    pub title: String,
    pub date: String,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub number: u32,
    pub gender: String,
    pub age_group: String,
    pub distance_m: u32,
    pub stroke: String,
    pub heats: Vec<Heat>,
}

#[derive(Debug, Clone)]
pub struct Heat {
    pub number: u32,
    pub of: u32,
    pub lanes: Vec<Lane>,
}

#[derive(Debug, Clone)]
pub struct Lane {
    pub number: u32,
    pub swimmer: Option<Swimmer>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Swimmer {
    pub last_name: String,
    pub first_name: String,
    pub age: u32,
    pub exhibition: bool,
    pub team: String,
    pub seed_time: SeedTime,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum SeedTime {
    NoTime,
    Seconds(f64),
}

impl fmt::Display for SeedTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeedTime::NoTime => write!(f, "NT"),
            SeedTime::Seconds(secs) if *secs >= 60.0 => {
                let minutes = (secs / 60.0).floor();
                let rem = secs - minutes * 60.0;
                write!(f, "{}:{:05.2}", minutes as u64, rem)
            }
            SeedTime::Seconds(secs) => write!(f, "{secs:.2}"),
        }
    }
}
