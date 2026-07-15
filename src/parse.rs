use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Event, Heat, Lane, Meet, SeedTime, Swimmer, CORRUPTION_MARKER};

/// A gap in the parse: either a line we didn't recognize, or a name/team the
/// PDF's font couldn't render (left behind as `CORRUPTION_MARKER`). Neither
/// stops ingestion; both are worth showing the user so they can validate the
/// import or add a correction.
#[derive(Debug, Clone)]
pub enum Issue {
    UnparsedLine { line: usize, text: String },
    UnresolvedCharacter { context: String },
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Issue::UnparsedLine { line, text } => {
                write!(f, "line {line}: couldn't parse: {text:?}")
            }
            Issue::UnresolvedCharacter { context } => {
                write!(
                    f,
                    "unresolved character ({CORRUPTION_MARKER}) in: {context}"
                )
            }
        }
    }
}

static HEADER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^Heat Sheet (?P<title>.+?) — (?P<date>.+?) Page \d+ of \d+$").unwrap()
});
static EVENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^#(?P<num>\d+) (?P<gender>\S+) (?P<age_group>.+?) (?P<dist>\d+)m (?P<stroke>.+)$")
        .unwrap()
});
static HEAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Heat (?P<n>\d+) of (?P<of>\d+)$").unwrap());
static LANE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<lane>\d+) (?:—$|(?P<last>[^,]+), (?P<first>.+?) (?:(?P<exh>EXH) )?(?P<age>\d+) (?P<team>.+?) (?P<time>(?:\d+:)?\d+\.\d{2}|NT))$",
    )
    .unwrap()
});

/// Replace every stray glyph gap left by the PDF's font with one consistent,
/// easy-to-paste marker, then fix the one word we know unambiguously
/// (Butterfly is a closed vocabulary item, always missing the same "fl").
/// Everything else is left for the caller to patch via a corrections file.
pub fn normalize_corruption(text: &str) -> String {
    let text = text.replace('\0', &CORRUPTION_MARKER.to_string());
    text.replace(&format!("Butter{CORRUPTION_MARKER}y"), "Butterfly")
}

/// Apply user-supplied literal find/replace pairs, in order, to patch names
/// the parser can't recover on its own (see `load_corrections`).
pub fn apply_corrections(text: &str, corrections: &[(String, String)]) -> String {
    let mut text = text.to_string();
    for (find, replace) in corrections {
        text = text.replace(find, replace);
    }
    text
}

fn collapse_whitespace(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_seed_time(s: &str) -> SeedTime {
    if s == "NT" {
        return SeedTime::NoTime;
    }
    match s.split_once(':') {
        Some((minutes, rest)) => {
            let minutes: f64 = minutes.parse().unwrap_or(0.0);
            let seconds: f64 = rest.parse().unwrap_or(0.0);
            SeedTime::Seconds(minutes * 60.0 + seconds)
        }
        None => SeedTime::Seconds(s.parse().unwrap_or(0.0)),
    }
}

#[derive(Default)]
struct Builder {
    events: Vec<Event>,
    current_event: Option<Event>,
    current_heat: Option<Heat>,
}

impl Builder {
    fn flush_heat(&mut self) {
        if let Some(heat) = self.current_heat.take() {
            if let Some(event) = self.current_event.as_mut() {
                event.heats.push(heat);
            }
        }
    }

    fn flush_event(&mut self) {
        self.flush_heat();
        if let Some(event) = self.current_event.take() {
            self.events.push(event);
        }
    }
}

/// Parse the (already corruption-normalized) text of a heat sheet into a
/// `Meet`, alongside any issues found so the caller can print them for
/// manual review.
pub fn parse_meet(text: &str) -> (Meet, Vec<Issue>) {
    let mut title = None;
    let mut date = None;
    let mut issues = Vec::new();
    let mut builder = Builder::default();

    for (line_number, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("SwimTopia") {
            continue;
        }
        let line = collapse_whitespace(line);

        if let Some(caps) = HEADER_RE.captures(&line) {
            title.get_or_insert_with(|| caps["title"].to_string());
            date.get_or_insert_with(|| caps["date"].to_string());
        } else if let Some(caps) = EVENT_RE.captures(&line) {
            builder.flush_event();
            builder.current_event = Some(Event {
                number: caps["num"].parse().unwrap_or(0),
                gender: caps["gender"].to_string(),
                age_group: caps["age_group"].to_string(),
                distance_m: caps["dist"].parse().unwrap_or(0),
                stroke: caps["stroke"].to_string(),
                heats: Vec::new(),
            });
        } else if let Some(caps) = HEAT_RE.captures(&line) {
            builder.flush_heat();
            builder.current_heat = Some(Heat {
                number: caps["n"].parse().unwrap_or(0),
                of: caps["of"].parse().unwrap_or(0),
                lanes: Vec::new(),
            });
        } else if let Some(caps) = LANE_RE.captures(&line) {
            let lane_number: u32 = caps["lane"].parse().unwrap_or(0);
            let swimmer = caps.name("last").map(|_| Swimmer {
                last_name: caps["last"].to_string(),
                first_name: caps["first"].to_string(),
                age: caps["age"].parse().unwrap_or(0),
                exhibition: caps.name("exh").is_some(),
                team: caps["team"].to_string(),
                seed_time: parse_seed_time(&caps["time"]),
            });
            if let Some(swimmer) = &swimmer {
                let context = format!(
                    "{}, {} ({})",
                    swimmer.last_name, swimmer.first_name, swimmer.team
                );
                if context.contains(CORRUPTION_MARKER) {
                    issues.push(Issue::UnresolvedCharacter { context });
                }
            }
            match builder.current_heat.as_mut() {
                Some(heat) => heat.lanes.push(Lane {
                    number: lane_number,
                    swimmer,
                }),
                None => issues.push(Issue::UnparsedLine {
                    line: line_number + 1,
                    text: raw_line.to_string(),
                }),
            }
        } else {
            issues.push(Issue::UnparsedLine {
                line: line_number + 1,
                text: raw_line.to_string(),
            });
        }
    }
    builder.flush_event();

    let meet = Meet {
        title: title.unwrap_or_default(),
        date: date.unwrap_or_default(),
        events: builder.events,
    };
    (meet, issues)
}
