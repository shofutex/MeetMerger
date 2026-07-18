use std::collections::BTreeMap;
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
static CSV_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<last>[^,]+), (?P<first>.+)$").unwrap());

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

fn csv_header_index(headers: &csv::StringRecord, matcher: impl Fn(&str) -> bool) -> Option<usize> {
    headers.iter().position(|h| matcher(&h.trim().to_lowercase()))
}

/// Parse a CSV of individual lane entries into a `Meet`. Expected columns
/// (matched case-insensitively, in any order): event name (in the same
/// "#N Gender AgeGroup Dm Stroke" form as the PDF heat sheet), a heat column
/// (e.g. "Heat 2 of 3"), lane, name ("Last, First", optionally suffixed
/// " EXH"), age, team, and entry time. Rows whose event/heat/name fields
/// don't match the expected shape are skipped and reported as issues, same
/// as unparsed lines from the PDF importer. The CSV has no title/date
/// header, so the caller supplies a title (typically the file name).
pub fn parse_meet_csv(data: &str, title: &str) -> (Meet, Vec<Issue>) {
    let mut issues = Vec::new();
    let mut events: BTreeMap<u32, Event> = BTreeMap::new();

    let mut reader = csv::ReaderBuilder::new().from_reader(data.as_bytes());
    let headers = match reader.headers() {
        Ok(headers) => headers.clone(),
        Err(err) => {
            issues.push(Issue::UnparsedLine {
                line: 1,
                text: format!("couldn't read CSV header: {err}"),
            });
            return (
                Meet {
                    title: title.to_string(),
                    date: String::new(),
                    events: Vec::new(),
                },
                issues,
            );
        }
    };

    let idx_event = csv_header_index(&headers, |h| h == "event name" || h == "event");
    let idx_heat = csv_header_index(&headers, |h| h.contains("heat"));
    let idx_lane = csv_header_index(&headers, |h| h == "lane");
    let idx_name = csv_header_index(&headers, |h| h == "name" || h == "swimmer");
    let idx_age = csv_header_index(&headers, |h| h == "age");
    let idx_team = csv_header_index(&headers, |h| h == "team");
    let idx_time = csv_header_index(&headers, |h| h.contains("time"));

    for (row_number, result) in reader.records().enumerate() {
        let line = row_number + 2; // header occupies line 1
        let Ok(record) = result else {
            issues.push(Issue::UnparsedLine {
                line,
                text: "couldn't parse CSV row".to_string(),
            });
            continue;
        };

        let field = |idx: Option<usize>| idx.and_then(|i| record.get(i)).unwrap_or("").trim();

        let event_name = field(idx_event);
        let heat_label = field(idx_heat);
        let lane_field = field(idx_lane);
        let name_field = field(idx_name);
        let age_field = field(idx_age);
        let team_field = field(idx_team);
        let time_field = field(idx_time);

        let Some(event_caps) = EVENT_RE.captures(event_name) else {
            issues.push(Issue::UnparsedLine {
                line,
                text: format!("couldn't parse event name: {event_name:?}"),
            });
            continue;
        };
        let Some(heat_caps) = HEAT_RE.captures(heat_label) else {
            issues.push(Issue::UnparsedLine {
                line,
                text: format!("couldn't parse heat: {heat_label:?}"),
            });
            continue;
        };

        let event_number: u32 = event_caps["num"].parse().unwrap_or(0);
        let event = events.entry(event_number).or_insert_with(|| Event {
            number: event_number,
            gender: event_caps["gender"].to_string(),
            age_group: event_caps["age_group"].to_string(),
            distance_m: event_caps["dist"].parse().unwrap_or(0),
            stroke: event_caps["stroke"].to_string(),
            heats: Vec::new(),
        });

        let heat_number: u32 = heat_caps["n"].parse().unwrap_or(0);
        let heat_of: u32 = heat_caps["of"].parse().unwrap_or(0);
        let heat_pos = match event.heats.iter().position(|h| h.number == heat_number) {
            Some(pos) => pos,
            None => {
                event.heats.push(Heat {
                    number: heat_number,
                    of: heat_of,
                    lanes: Vec::new(),
                });
                event.heats.len() - 1
            }
        };

        let swimmer = if name_field.is_empty() || name_field == "—" || name_field == "-" {
            None
        } else {
            match CSV_NAME_RE.captures(name_field) {
                Some(caps) => {
                    let mut first = caps["first"].trim();
                    let mut exhibition = false;
                    if let Some(stripped) = first.strip_suffix("EXH") {
                        exhibition = true;
                        first = stripped.trim();
                    }
                    Some(Swimmer {
                        last_name: caps["last"].trim().to_string(),
                        first_name: first.to_string(),
                        age: age_field.parse().unwrap_or(0),
                        exhibition,
                        team: team_field.to_string(),
                        seed_time: parse_seed_time(time_field),
                    })
                }
                None => {
                    issues.push(Issue::UnparsedLine {
                        line,
                        text: format!("couldn't parse name: {name_field:?}"),
                    });
                    None
                }
            }
        };

        event.heats[heat_pos].lanes.push(Lane {
            number: lane_field.parse().unwrap_or(0),
            swimmer,
        });
    }

    for event in events.values_mut() {
        event.heats.sort_by_key(|h| h.number);
        for heat in &mut event.heats {
            heat.lanes.sort_by_key(|l| l.number);
        }
    }

    let meet = Meet {
        title: title.to_string(),
        date: String::new(),
        events: events.into_values().collect(),
    };
    (meet, issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meet_csv_builds_events_heats_and_lanes_regardless_of_row_order() {
        let data = "\
event name,heat,lane,name,age,team,entry time
#2 Girls 10 & Under 50m Freestyle,Heat 1 of 1,4,\"Doe, Jane\",9,Sharks,32.10
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 2,3,\"Smith, John\",7,Dolphins,NT
#1 Boys 8 & Under 25m Freestyle,Heat 2 of 2,1,\"Roe, Sam EXH\",8,Dolphins,1:02.34
";
        let (meet, issues) = parse_meet_csv(data, "My Meet");
        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert_eq!(meet.title, "My Meet");
        assert_eq!(meet.events.len(), 2);

        let event1 = &meet.events[0];
        assert_eq!(event1.number, 1);
        assert_eq!(event1.gender, "Boys");
        assert_eq!(event1.heats.len(), 2);
        let heat1 = &event1.heats[0];
        assert_eq!(heat1.number, 1);
        assert_eq!(heat1.of, 2);
        assert_eq!(heat1.lanes[0].number, 3);
        let swimmer = heat1.lanes[0].swimmer.as_ref().unwrap();
        assert_eq!(swimmer.last_name, "Smith");
        assert_eq!(swimmer.first_name, "John");
        assert_eq!(swimmer.seed_time, SeedTime::NoTime);

        let heat2 = &event1.heats[1];
        let exh_swimmer = heat2.lanes[0].swimmer.as_ref().unwrap();
        assert_eq!(exh_swimmer.first_name, "Sam");
        assert!(exh_swimmer.exhibition);

        let event2 = &meet.events[1];
        assert_eq!(event2.number, 2);
        assert_eq!(event2.heats[0].lanes[0].number, 4);
    }

    #[test]
    fn parse_meet_csv_reports_unparseable_event_name_as_an_issue() {
        let data = "\
event name,heat,lane,name,age,team,entry time
Not A Real Event,Heat 1 of 1,1,\"Doe, Jane\",9,Sharks,32.10
";
        let (meet, issues) = parse_meet_csv(data, "My Meet");
        assert!(meet.events.is_empty());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn parse_meet_csv_treats_a_dash_name_as_an_empty_lane() {
        let data = "\
event name,heat,lane,name,age,team,entry time
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,2,—,,,
";
        let (meet, issues) = parse_meet_csv(data, "My Meet");
        assert!(issues.is_empty());
        assert!(meet.events[0].heats[0].lanes[0].swimmer.is_none());
    }
}
