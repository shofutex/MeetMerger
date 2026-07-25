use std::collections::BTreeMap;
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Event, Heat, Lane, Meet, Record, SeedTime, Swimmer, CORRUPTION_MARKER};

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
// A team acronym is always an all-caps (and/or digit) token, e.g. "FO",
// "VAC", "L", "NVSL" — unlike a team's full name, which is title-case and
// always has at least one lowercase letter. This is what lets the record
// scanner below tell "the start of the next record" apart from "the
// previous record's team name line".
const ACRONYM_RE_FRAGMENT: &str = r"[A-Z][A-Z0-9]*";
// A one-line record: acronym, swimmer name ("First Last" order, unlike the
// "Last, First" lane rows use), year, and time all on one line.
static ONE_LINE_RECORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?P<acronym>{ACRONYM_RE_FRAGMENT}) (?P<name>.+?) (?P<year>\d{{4}}) (?P<time>(?:\d+:)?\d+\.\d{{2}})$"
    ))
    .unwrap()
});
// A record's name line with no year/time attached yet — the rest (a team
// name, and/or the year and time) follows on the next one or two lines.
static RECORD_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^(?P<acronym>{ACRONYM_RE_FRAGMENT}) (?P<name>.+)$")).unwrap());
static RECORD_YEAR_TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<year>\d{4}) (?P<time>(?:\d+:)?\d+\.\d{2})$").unwrap()
});
static RECORD_YEAR_ONLY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<year>\d{4})$").unwrap());
static RECORD_TIME_ONLY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\d+:)?\d+\.\d{2}$").unwrap());

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
    // Between an event's header and its first heat, the heat sheet may show
    // one standing-record block per team, back to back, each spanning one to
    // three lines (see `resolve_pending_record`). Lines are buffered here
    // until we know whether a heat or another event line ends the window,
    // since we can't tell how many records are present from the first line
    // alone.
    awaiting_record: bool,
    record_lines: Vec<(usize, String)>,
    // An "Alternates" list follows an event's heats; every line until the
    // next event header is noise we intentionally drop.
    skipping_alternates: bool,
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

    fn start_record_window(&mut self) {
        self.awaiting_record = true;
        self.record_lines.clear();
    }

    // Resolves the buffered record-window lines (if any) against the current
    // event, once a heat or the next event line closes the window. Scans
    // sequentially rather than assuming a fixed line count, since each
    // team's record block independently takes one of a few shapes:
    //   - one line: "acronym name year time"
    //   - two lines: "acronym name" then "year time" (no team name shown)
    //   - three lines: "acronym name" then "team name" then "year time"
    //   - three lines: "acronym name" then "year" then "time" (no team name,
    //     year and time on their own lines)
    // A line that doesn't fit the start of any of these shapes — including
    // one left dangling with no year/time to follow it — is reported as an
    // issue instead of silently dropped.
    fn resolve_pending_record(&mut self, issues: &mut Vec<Issue>) {
        if !self.awaiting_record {
            return;
        }
        self.awaiting_record = false;
        let lines = std::mem::take(&mut self.record_lines);
        let mut records = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let (line_number, text) = &lines[i];
            if let Some(caps) = ONE_LINE_RECORD_RE.captures(text) {
                records.push(Record {
                    team_acronym: caps["acronym"].to_string(),
                    swimmer_name: caps["name"].to_string(),
                    team_name: String::new(),
                    year: caps["year"].parse().unwrap_or(0),
                    time: caps["time"].to_string(),
                });
                i += 1;
                continue;
            }
            let Some(name_caps) = RECORD_NAME_RE.captures(text) else {
                issues.push(Issue::UnparsedLine {
                    line: *line_number,
                    text: text.clone(),
                });
                i += 1;
                continue;
            };
            let team_acronym = name_caps["acronym"].to_string();
            let swimmer_name = name_caps["name"].to_string();
            if let Some(next) = lines.get(i + 1).and_then(|(_, t)| RECORD_YEAR_TIME_RE.captures(t))
            {
                records.push(Record {
                    team_acronym,
                    swimmer_name,
                    team_name: String::new(),
                    year: next["year"].parse().unwrap_or(0),
                    time: next["time"].to_string(),
                });
                i += 2;
            } else if lines
                .get(i + 1)
                .is_some_and(|(_, t)| RECORD_YEAR_ONLY_RE.is_match(t))
                && lines
                    .get(i + 2)
                    .is_some_and(|(_, t)| RECORD_TIME_ONLY_RE.is_match(t))
            {
                records.push(Record {
                    team_acronym,
                    swimmer_name,
                    team_name: String::new(),
                    year: lines[i + 1].1.parse().unwrap_or(0),
                    time: lines[i + 2].1.clone(),
                });
                i += 3;
            } else if let Some(time_caps) = lines
                .get(i + 2)
                .and_then(|(_, t)| RECORD_YEAR_TIME_RE.captures(t))
            {
                records.push(Record {
                    team_acronym,
                    swimmer_name,
                    team_name: lines[i + 1].1.clone(),
                    year: time_caps["year"].parse().unwrap_or(0),
                    time: time_caps["time"].to_string(),
                });
                i += 3;
            } else {
                issues.push(Issue::UnparsedLine {
                    line: *line_number,
                    text: text.clone(),
                });
                i += 1;
            }
        }
        if let Some(event) = self.current_event.as_mut() {
            event.records = records;
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

        if builder.skipping_alternates {
            if EVENT_RE.is_match(&line) {
                builder.skipping_alternates = false;
            } else {
                continue;
            }
        }

        if let Some(caps) = HEADER_RE.captures(&line) {
            title.get_or_insert_with(|| caps["title"].to_string());
            date.get_or_insert_with(|| caps["date"].to_string());
        } else if let Some(caps) = EVENT_RE.captures(&line) {
            builder.resolve_pending_record(&mut issues);
            builder.flush_event();
            builder.current_event = Some(Event {
                number: caps["num"].parse().unwrap_or(0),
                gender: caps["gender"].to_string(),
                age_group: caps["age_group"].to_string(),
                distance_m: caps["dist"].parse().unwrap_or(0),
                stroke: caps["stroke"].to_string(),
                heats: Vec::new(),
                records: Vec::new(),
            });
            builder.start_record_window();
        } else if let Some(caps) = HEAT_RE.captures(&line) {
            builder.resolve_pending_record(&mut issues);
            builder.flush_heat();
            builder.current_heat = Some(Heat {
                number: caps["n"].parse().unwrap_or(0),
                of: caps["of"].parse().unwrap_or(0),
                lanes: Vec::new(),
            });
        } else if line == "Alternates" {
            builder.skipping_alternates = true;
        } else if builder.awaiting_record {
            builder.record_lines.push((line_number + 1, line));
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
    builder.resolve_pending_record(&mut issues);
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
///
/// Five more columns are optional: "record team", "record name", "record
/// year", "record time", "record team name". An event can hold more than one
/// team's record — fill them in on one row per team (any other rows are
/// blank); every row with all five non-empty adds one record to that event.
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
    let idx_time = csv_header_index(&headers, |h| h.contains("time") && !h.contains("record"));
    let idx_record_team = csv_header_index(&headers, |h| h == "record team");
    let idx_record_name = csv_header_index(&headers, |h| h == "record name");
    let idx_record_year = csv_header_index(&headers, |h| h == "record year");
    let idx_record_time = csv_header_index(&headers, |h| h == "record time");
    let idx_record_team_name = csv_header_index(&headers, |h| h == "record team name");

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
        let record_team_field = field(idx_record_team);
        let record_name_field = field(idx_record_name);
        let record_year_field = field(idx_record_year);
        let record_time_field = field(idx_record_time);
        let record_team_name_field = field(idx_record_team_name);

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
            records: Vec::new(),
        });

        if !record_team_field.is_empty()
            && !record_name_field.is_empty()
            && !record_year_field.is_empty()
            && !record_time_field.is_empty()
            && !record_team_name_field.is_empty()
        {
            let record = Record {
                team_acronym: record_team_field.to_string(),
                swimmer_name: record_name_field.to_string(),
                year: record_year_field.parse().unwrap_or(0),
                time: record_time_field.to_string(),
                team_name: record_team_name_field.to_string(),
            };
            if !event.records.contains(&record) {
                event.records.push(record);
            }
        }

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
#[path = "parse/tests.rs"]
mod tests;
