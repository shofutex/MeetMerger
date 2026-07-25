use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use printpdf::*;

use crate::merge::{MixedHeat, MixedHeatSource};
use crate::model::{Event, Lane, Meet, Record, SeedTime, Swimmer};

// US Letter portrait, in millimeters.
const PAGE_W: f32 = 215.9;
const PAGE_H: f32 = 279.4;
const MARGIN: f32 = 12.0;
const HEADER_HEIGHT: f32 = 20.0;
const GUTTER: f32 = 6.0;
const COLUMNS: usize = 3;

const HEADER_TEXT_Y: f32 = PAGE_H - MARGIN - 6.0;
const HEADER_DIVIDER_Y: f32 = PAGE_H - MARGIN - HEADER_HEIGHT + 6.0;
const CONTENT_TOP: f32 = PAGE_H - MARGIN - HEADER_HEIGHT;
const COLUMN_HEIGHT: f32 = CONTENT_TOP - MARGIN;
const COL_WIDTH: f32 = (PAGE_W - 2.0 * MARGIN - (COLUMNS as f32 - 1.0) * GUTTER) / COLUMNS as f32;

const EVENT_LINE_H: f32 = 5.0;
const DIVIDER_LINE_H: f32 = 3.0;
const HEAT_LABEL_LINE_H: f32 = 4.2;
const SWIMMER_LINE_H: f32 = 4.2;
const EVENT_GAP_H: f32 = 6.0;

// Fixed x-offsets (mm) of each swimmer field from a column's left edge.
// printpdf's Base-14 fonts have no string-width measurement, so these are
// hardcoded rather than computed; long names/teams may run close to the
// next field. Tune against a rendered sample rather than trusting this
// blind.
const LANE_X: f32 = 0.0;
const NAME_X: f32 = 6.0;
const EXH_X: f32 = 26.0;
// "EXH" in 4.5pt Helvetica-Bold measures ~3.35mm (Adobe AFM widths E=667,
// X=722, H=722 per 1000 units-per-em); with the text starting 0.9mm from
// the box's left edge, matching that same 0.9mm gap on the right needs a
// box this wide. Re-derive if the font/size/left-offset ever changes.
const EXH_BOX_W: f32 = 5.15;
const EXH_BOX_H: f32 = 2.8;
const EXH_BOX_RADIUS: f32 = 0.7;
// Age is right-justified so single- and double-digit ages line up on their
// ones digit. Right edge of the age slot, 2mm before the team field starts.
const AGE_RIGHT_X: f32 = 36.0;
const TEAM_X: f32 = 38.0;
// Adobe AFM: every digit 0-9 in Helvetica is exactly 556/1000 em wide, so an
// age's on-page width is computable without printpdf's (nonexistent) string
// measurement — no font-metrics table needed for numeric text.
const HELVETICA_DIGIT_WIDTH_EM: f32 = 0.556;
const PT_TO_MM: f32 = 25.4 / 72.0;

fn digits_width_mm(digits: &str, size_pt: f32) -> f32 {
    digits.chars().count() as f32 * HELVETICA_DIGIT_WIDTH_EM * size_pt * PT_TO_MM
}
const TIME_X: f32 = 52.0;

// A record row mirrors the swimmer row's columns: acronym box at LANE_X,
// bold name at NAME_X, bold year right-justified at AGE_RIGHT_X, bold time
// at TIME_X. A long name wraps onto its own line, pushing year/time to a
// second line, the same way a long swimmer name pushes age/team/time down.
const RECORD_BOX_W: f32 = 5.5;
const RECORD_BOX_H: f32 = 3.6;
const RECORD_ACRONYM_SIZE: f32 = 5.5;
const RECORD_LINE_H: f32 = SWIMMER_LINE_H;
// A record name longer than this (e.g. "John Paul Gonsalves") would run into
// the year column at AGE_RIGHT_X, so it gets its own line instead, same
// spirit as `name_wrap_threshold` for swimmer rows.
const RECORD_NAME_WRAP_THRESHOLD: usize = 17;

// "Last, First" longer than this wraps the first name onto its own line so
// the EXH badge has room next to whatever's left on the name's line. Only
// exhibition swimmers need that room, so non-exhibition swimmers get a
// longer threshold before wrapping.
const NAME_WRAP_THRESHOLD_EXH: usize = 15;
const NAME_WRAP_THRESHOLD_OTHER: usize = 22;

fn name_wrap_threshold(exhibition: bool) -> usize {
    if exhibition {
        NAME_WRAP_THRESHOLD_EXH
    } else {
        NAME_WRAP_THRESHOLD_OTHER
    }
}

// Event names 48 characters or longer wrap onto a second line so they don't
// run past the column/page width.
const EVENT_NAME_WRAP_THRESHOLD: usize = 48;

// Splits at the space nearest the midpoint. Falls back to no wrap if there's
// no space to split on (e.g. a single very long word).
fn wrap_event_name(name: &str) -> (&str, Option<&str>) {
    if name.len() < EVENT_NAME_WRAP_THRESHOLD {
        return (name, None);
    }
    let mid = name.len() / 2;
    let split = name[..mid]
        .rfind(' ')
        .or_else(|| name[mid..].find(' ').map(|i| mid + i));
    match split {
        Some(idx) => (name[..idx].trim_end(), Some(name[idx..].trim_start())),
        None => (name, None),
    }
}

// Timer sheets print single-column, full page width, one page per lane.
const TIMER_CONTENT_WIDTH: f32 = PAGE_W - 2.0 * MARGIN;
const TIMER_LANE_X: f32 = 0.0;
const TIMER_HEAT_X: f32 = 16.0;
// Base position (38.0mm) plus 10 space-widths in 7pt Helvetica (Adobe AFM:
// space = 278/1000 em -> 1.946pt -> 0.6865mm; x10 = 6.865mm), per the request
// to move swimmer names 10 spaces to the right. Re-derive if the row font
// size ever changes.
const TIMER_NAME_X: f32 = 38.0 + 6.865;
const TIMER_TEAM_X: f32 = 120.0;
const TIMER_BLANKS_X: f32 = 136.0;
const TIMER_BLANK_COUNT: usize = 4;
const TIMER_BLANK_GAP: f32 = 3.0;

const TIMER_EVENT_LINE_H: f32 = 5.0;
// Extra room between the event divider and the first heat row underneath it.
const TIMER_DIVIDER_LINE_H: f32 = 7.0;
const TIMER_ROW_H: f32 = 5.5;
const TIMER_ROW_GAP_H: f32 = 2.5;
const TIMER_EVENT_GAP_H: f32 = 8.0;

pub fn rotate_events(events: &[Event], start_event: u32) -> Vec<&Event> {
    let split = events
        .iter()
        .position(|e| e.number >= start_event)
        .unwrap_or(events.len());
    events[split..]
        .iter()
        .chain(events[..split].iter())
        .collect()
}

pub struct PrintSwimmer {
    pub lane: u32,
    pub last_name: String,
    pub first_name: String,
    pub age: u32,
    pub team: String,
    pub exhibition: bool,
    pub seed_time: SeedTime,
}

fn full_name_len(last: &str, first: &str) -> usize {
    last.len() + 2 + first.len() // ", " separator
}

pub struct PrintHeat {
    pub heat_label: String,
    pub swimmers: Vec<PrintSwimmer>,
}

pub struct PrintEvent {
    pub event_name: String,
    pub heats: Vec<PrintHeat>,
    // One entry per team holding a standing record for this event.
    pub records: Vec<PrintRecord>,
    // The originating event's number — the anchor event's, for a mixed
    // heat's synthesized block. Used to let the page-break-before-event
    // option find where to start a fresh page, regardless of rotation.
    pub number: u32,
}

pub struct PrintRecord {
    pub team_acronym: String,
    pub swimmer_name: String,
    pub year: u32,
    pub time: String,
}

fn abbreviate<'a>(team: &'a str, abbreviations: &'a HashMap<String, String>) -> &'a str {
    abbreviations
        .get(team)
        .map(String::as_str)
        .filter(|a| !a.is_empty())
        .unwrap_or(team)
}

fn swimmer_rows(lanes: &[Lane], abbreviations: &HashMap<String, String>) -> Vec<PrintSwimmer> {
    lanes
        .iter()
        .filter_map(|lane| {
            lane.swimmer.as_ref().map(|s| PrintSwimmer {
                lane: lane.number,
                last_name: s.last_name.clone(),
                first_name: s.first_name.clone(),
                age: s.age,
                team: abbreviate(&s.team, abbreviations).to_string(),
                exhibition: s.exhibition,
                seed_time: s.seed_time,
            })
        })
        .collect()
}

fn event_name(event: &Event) -> String {
    format!(
        "#{} {} {} {}m {}",
        event.number, event.gender, event.age_group, event.distance_m, event.stroke
    )
}

// Every distinct team name appearing in the printable result (remaining
// original heats plus mixed heats), sorted, for the abbreviation picker.
pub fn distinct_teams(
    meet: &Meet,
    consumed: &HashSet<(u32, u32)>,
    mixed_heats: &[MixedHeat],
) -> Vec<String> {
    let mut teams = BTreeSet::new();
    for event in &meet.events {
        for heat in &event.heats {
            if consumed.contains(&(event.number, heat.number)) {
                continue;
            }
            for lane in &heat.lanes {
                if let Some(s) = &lane.swimmer {
                    teams.insert(s.team.clone());
                }
            }
        }
    }
    for mixed in mixed_heats {
        for lane in &mixed.lanes {
            if let Some(s) = &lane.swimmer {
                teams.insert(s.team.clone());
            }
        }
    }
    teams.into_iter().collect()
}

// Splits a flat mixed_heats list back into the runs one merge produced: the
// heats a single merge action creates are always pushed contiguously and in
// heat_index order, so each run is just the next heat_count entries.
fn mixed_heat_groups(mixed_heats: &[MixedHeat]) -> Vec<&[MixedHeat]> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < mixed_heats.len() {
        let end = (i + mixed_heats[i].heat_count.max(1)).min(mixed_heats.len());
        groups.push(&mixed_heats[i..end]);
        i = end;
    }
    groups
}

pub struct ChangeRow {
    pub assigned_lane: u32,
    pub swimmer_name: String,
    pub original_event: u32,
    pub original_heat: u32,
    pub original_lane: u32,
}

pub struct ChangeHeat {
    pub heat_label: String,
    pub rows: Vec<ChangeRow>,
}

pub struct ChangeEvent {
    pub event_name: String,
    pub heats: Vec<ChangeHeat>,
}

// Finds where a swimmer now sitting in a mixed heat originally raced, by
// checking each of the mixed heat's source heats for a matching swimmer.
fn find_original_lane(
    meet: &Meet,
    sources: &[MixedHeatSource],
    swimmer: &Swimmer,
) -> Option<(u32, u32, u32)> {
    sources.iter().find_map(|source| {
        let lane = meet
            .events
            .iter()
            .find(|e| e.number == source.event_number)
            .and_then(|e| e.heats.iter().find(|h| h.number == source.heat_number))
            .and_then(|h| h.lanes.iter().find(|l| l.swimmer.as_ref() == Some(swimmer)))?;
        Some((source.event_number, source.heat_number, lane.number))
    })
}

// A flat report of every swimmer moved into a mixed heat: their newly
// assigned lane alongside where they raced originally, so a human can
// sanity-check the merge before printing.
pub fn build_changes(meet: &Meet, mixed_heats: &[MixedHeat]) -> Vec<ChangeEvent> {
    mixed_heat_groups(mixed_heats)
        .into_iter()
        .filter_map(|group| {
            let first = group.first()?;
            let heats: Vec<ChangeHeat> = group
                .iter()
                .map(|mixed| ChangeHeat {
                    heat_label: format!("Heat {} of {}", mixed.heat_index, mixed.heat_count),
                    rows: mixed
                        .lanes
                        .iter()
                        .filter_map(|lane| {
                            let swimmer = lane.swimmer.as_ref()?;
                            let (original_event, original_heat, original_lane) =
                                find_original_lane(meet, &mixed.sources, swimmer)?;
                            Some(ChangeRow {
                                assigned_lane: lane.number,
                                swimmer_name: format!(
                                    "{}, {}",
                                    swimmer.last_name, swimmer.first_name
                                ),
                                original_event,
                                original_heat,
                                original_lane,
                            })
                        })
                        .collect(),
                })
                .collect();
            Some(ChangeEvent {
                event_name: first.header.clone(),
                heats,
            })
        })
        .collect()
}

fn print_record(record: &Record) -> PrintRecord {
    PrintRecord {
        team_acronym: record.team_acronym.clone(),
        swimmer_name: record.swimmer_name.clone(),
        year: record.year,
        time: record.time.clone(),
    }
}

// Walks events in rotated print order; for each event, emits one PrintEvent
// holding every remaining (non-consumed) heat, then interleaves any mixed
// heats anchored to that event number, mirroring the GUI's Final Preview
// ordering. A mixed heat's splits are treated like a normal event's heats:
// one PrintEvent, its name shown once, holding every split underneath.
// Skips events left with no remaining heats. `show_records` controls whether
// an event's records (if the source heat sheet had any — one per team that
// holds a standing record) are attached to its PrintEvent; a mixed heat's
// synthesized PrintEvent never carries records, since it may draw from more
// than one original event.
pub fn build_print_events(
    meet: &Meet,
    consumed: &HashSet<(u32, u32)>,
    mixed_heats: &[MixedHeat],
    abbreviations: &HashMap<String, String>,
    start_event: u32,
    show_records: bool,
) -> Vec<PrintEvent> {
    let mixed_groups = mixed_heat_groups(mixed_heats);
    let mut events = Vec::new();
    for event in rotate_events(&meet.events, start_event) {
        let heats: Vec<PrintHeat> = event
            .heats
            .iter()
            .filter(|h| !consumed.contains(&(event.number, h.number)))
            .map(|h| PrintHeat {
                heat_label: format!("Heat {} of {}", h.number, h.of),
                swimmers: swimmer_rows(&h.lanes, abbreviations),
            })
            .collect();
        if !heats.is_empty() {
            events.push(PrintEvent {
                event_name: event_name(event),
                heats,
                records: if show_records {
                    event.records.iter().map(print_record).collect()
                } else {
                    Vec::new()
                },
                number: event.number,
            });
        }

        for group in &mixed_groups {
            let Some(first) = group.first() else {
                continue;
            };
            if first.anchor_event() == event.number {
                let heats: Vec<PrintHeat> = group
                    .iter()
                    .map(|mixed| PrintHeat {
                        heat_label: format!("Heat {} of {}", mixed.heat_index, mixed.heat_count),
                        swimmers: swimmer_rows(&mixed.lanes, abbreviations),
                    })
                    .collect();
                events.push(PrintEvent {
                    event_name: first.header.clone(),
                    heats,
                    records: Vec::new(),
                    number: first.anchor_event(),
                });
            }
        }
    }
    events
}

pub struct TimerSwimmer {
    pub last_name: String,
    pub first_name: String,
    pub age: u32,
    pub team: String,
}

pub struct TimerRow {
    pub heat_label: String,
    // None when this lane is empty for this heat — still printed as "No
    // swimmer" so a timer can follow every heat of the event, not just the
    // ones where their lane races.
    pub swimmer: Option<TimerSwimmer>,
}

pub struct TimerEvent {
    pub event_name: String,
    pub rows: Vec<TimerRow>,
}

pub struct TimerPage {
    pub lane: u32,
    pub events: Vec<TimerEvent>,
}

// One page per lane (1..=lane_capacity), every event and every one of its
// heats, in the same print order as the heat sheet. A heat where this lane
// has no swimmer still gets a row (with no `TimerSwimmer`) rather than being
// skipped, so the timer can track along heat by heat.
pub fn build_timer_pages(events: &[PrintEvent], lane_capacity: u32) -> Vec<TimerPage> {
    (1..=lane_capacity)
        .map(|lane| {
            let timer_events = events
                .iter()
                .map(|event| {
                    let rows: Vec<TimerRow> = event
                        .heats
                        .iter()
                        .map(|heat| TimerRow {
                            heat_label: heat.heat_label.clone(),
                            swimmer: heat.swimmers.iter().find(|s| s.lane == lane).map(|s| {
                                TimerSwimmer {
                                    last_name: s.last_name.clone(),
                                    first_name: s.first_name.clone(),
                                    age: s.age,
                                    team: s.team.clone(),
                                }
                            }),
                        })
                        .collect();
                    TimerEvent {
                        event_name: event.event_name.clone(),
                        rows,
                    }
                })
                .collect();
            TimerPage {
                lane,
                events: timer_events,
            }
        })
        .collect()
}

enum TimerLine<'a> {
    // Second line present when the event name wraps.
    EventName(&'a str, Option<&'a str>),
    Divider,
    Row(&'a str, Option<(&'a str, &'a str, u32, &'a str)>),
    RowGap,
    EventGap,
}

impl TimerLine<'_> {
    fn height(&self) -> f32 {
        match self {
            TimerLine::EventName(_, second) => {
                if second.is_some() {
                    TIMER_EVENT_LINE_H * 2.0
                } else {
                    TIMER_EVENT_LINE_H
                }
            }
            TimerLine::Divider => TIMER_DIVIDER_LINE_H,
            TimerLine::Row(..) => TIMER_ROW_H,
            TimerLine::RowGap => TIMER_ROW_GAP_H,
            TimerLine::EventGap => TIMER_EVENT_GAP_H,
        }
    }
}

// Packs a lane's events into physical pages: a page break happens right
// before a row that would overflow the page height, or (if given) once the
// page already holds `heats_per_page` rows — whichever comes first. Breaks
// only ever fall between rows, never mid-row. If a break lands in the
// middle of an event, the event's name and divider are repeated at the top
// of the next page, exactly as if that row were the event's first.
fn pack_timer_pages(events: &[TimerEvent], heats_per_page: Option<u32>) -> Vec<Vec<TimerLine<'_>>> {
    let mut pages: Vec<Vec<TimerLine<'_>>> = Vec::new();
    let mut current: Vec<TimerLine<'_>> = Vec::new();
    let mut used = 0.0f32;
    let mut heats_used = 0usize;

    for event in events {
        let (name_first, name_second) = wrap_event_name(&event.event_name);
        let name_h = if name_second.is_some() {
            TIMER_EVENT_LINE_H * 2.0
        } else {
            TIMER_EVENT_LINE_H
        };
        let full_header_h = name_h + TIMER_DIVIDER_LINE_H;

        let mut need_header = true;
        let last = event.rows.len().saturating_sub(1);
        for (index, row) in event.rows.iter().enumerate() {
            let header_h = if need_header { full_header_h } else { 0.0 };
            let over_height = used + header_h + TIMER_ROW_H > COLUMN_HEIGHT;
            let over_count = heats_per_page.is_some_and(|max| heats_used + 1 > max as usize);
            if (over_height || over_count) && !current.is_empty() {
                pages.push(std::mem::take(&mut current));
                used = 0.0;
                heats_used = 0;
                need_header = true;
            }

            if need_header {
                current.push(TimerLine::EventName(name_first, name_second));
                current.push(TimerLine::Divider);
                used += full_header_h;
                need_header = false;
            }

            current.push(TimerLine::Row(
                &row.heat_label,
                row.swimmer.as_ref().map(|s| {
                    (
                        s.last_name.as_str(),
                        s.first_name.as_str(),
                        s.age,
                        s.team.as_str(),
                    )
                }),
            ));
            used += TIMER_ROW_H;
            heats_used += 1;

            let gap = if index == last {
                TimerLine::EventGap
            } else {
                TimerLine::RowGap
            };
            used += gap.height();
            current.push(gap);
        }
    }
    if !current.is_empty() {
        pages.push(current);
    }
    pages
}

fn emit_timer_page(ops: &mut Vec<Op>, lane: u32, lines: &[TimerLine<'_>]) {
    let blank_width =
        (TIMER_CONTENT_WIDTH - TIMER_BLANKS_X - (TIMER_BLANK_COUNT as f32 - 1.0) * TIMER_BLANK_GAP)
            / TIMER_BLANK_COUNT as f32;

    let mut y = CONTENT_TOP;
    for line in lines {
        match line {
            TimerLine::EventName(first_line, second_line) => {
                show_text_at(ops, BuiltinFont::HelveticaBold, 8.0, MARGIN, y, first_line);
                if let Some(second_line) = second_line {
                    show_text_at(
                        ops,
                        BuiltinFont::HelveticaBold,
                        8.0,
                        MARGIN,
                        y - TIMER_EVENT_LINE_H,
                        second_line,
                    );
                }
            }
            TimerLine::Divider => {
                draw_hline(ops, MARGIN, PAGE_W - MARGIN, y, 0.5, rgb(0.5, 0.5, 0.5));
            }
            TimerLine::Row(heat_label, swimmer) => {
                show_text_at(
                    ops,
                    BuiltinFont::Helvetica,
                    7.0,
                    MARGIN + TIMER_LANE_X,
                    y,
                    &format!("Lane {lane}"),
                );
                show_text_at(
                    ops,
                    BuiltinFont::Helvetica,
                    7.0,
                    MARGIN + TIMER_HEAT_X,
                    y,
                    heat_label,
                );
                match swimmer {
                    Some((last, first, age, team)) => {
                        show_text_at(
                            ops,
                            BuiltinFont::Helvetica,
                            7.0,
                            MARGIN + TIMER_NAME_X,
                            y,
                            &format!("{last}, {first} ({age})"),
                        );
                        show_text_at(
                            ops,
                            BuiltinFont::Helvetica,
                            7.0,
                            MARGIN + TIMER_TEAM_X,
                            y,
                            team,
                        );
                    }
                    None => {
                        show_text_at(
                            ops,
                            BuiltinFont::HelveticaOblique,
                            7.0,
                            MARGIN + TIMER_NAME_X,
                            y,
                            "No swimmer",
                        );
                    }
                }

                for i in 0..TIMER_BLANK_COUNT {
                    let x_start =
                        MARGIN + TIMER_BLANKS_X + i as f32 * (blank_width + TIMER_BLANK_GAP);
                    draw_hline(
                        ops,
                        x_start,
                        x_start + blank_width,
                        y - 0.5,
                        0.5,
                        rgb(0.0, 0.0, 0.0),
                    );
                }
            }
            TimerLine::RowGap | TimerLine::EventGap => {}
        }
        y -= line.height();
    }
}

pub fn write_timer_pdf(
    meet_title: &str,
    pages: &[TimerPage],
    heats_per_page: Option<u32>,
    path: &Path,
) -> Result<(), String> {
    // Each lane packs independently so a lane always starts a fresh page,
    // even if the previous lane's last page had room to spare.
    let per_lane: Vec<(u32, Vec<Vec<TimerLine<'_>>>)> = pages
        .iter()
        .map(|page| (page.lane, pack_timer_pages(&page.events, heats_per_page)))
        .collect();

    let total_pages: usize = per_lane
        .iter()
        .map(|(_, lane_pages)| lane_pages.len().max(1))
        .sum();

    let mut doc = PdfDocument::new(meet_title);
    let mut pdf_pages = Vec::new();
    let mut page_number = 1usize;
    for (lane, lane_pages) in &per_lane {
        if lane_pages.is_empty() {
            let mut ops = Vec::new();
            emit_header(
                &mut ops,
                "Timer Sheets",
                meet_title,
                page_number,
                total_pages,
            );
            show_text_at(
                &mut ops,
                BuiltinFont::Helvetica,
                8.0,
                MARGIN,
                CONTENT_TOP,
                &format!("Lane {lane}: no events"),
            );
            pdf_pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
            page_number += 1;
            continue;
        }
        for lines in lane_pages {
            let mut ops = Vec::new();
            emit_header(
                &mut ops,
                "Timer Sheets",
                meet_title,
                page_number,
                total_pages,
            );
            emit_timer_page(&mut ops, *lane, lines);
            pdf_pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
            page_number += 1;
        }
    }
    doc.with_pages(pdf_pages);

    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

enum PrintLine<'a> {
    // Second line present when the event name wraps.
    EventName(&'a str, Option<&'a str>),
    Divider,
    Record(&'a PrintRecord),
    HeatLabel(&'a str),
    // (lane, last_name, first_name, age, team, exhibition, entry time text —
    // Some(text) prints it, None draws the hand-timing blank line instead)
    Swimmer(u32, &'a str, &'a str, u32, &'a str, bool, Option<String>),
    Gap,
}

impl PrintLine<'_> {
    fn height(&self) -> f32 {
        match self {
            PrintLine::EventName(_, second) => {
                if second.is_some() {
                    EVENT_LINE_H * 2.0
                } else {
                    EVENT_LINE_H
                }
            }
            PrintLine::Divider => DIVIDER_LINE_H,
            PrintLine::Record(record) => {
                if record.swimmer_name.chars().count() > RECORD_NAME_WRAP_THRESHOLD {
                    RECORD_LINE_H * 2.0
                } else {
                    RECORD_LINE_H
                }
            }
            PrintLine::HeatLabel(_) => HEAT_LABEL_LINE_H,
            PrintLine::Swimmer(_, last, first, _, _, exhibition, _) => {
                if full_name_len(last, first) > name_wrap_threshold(*exhibition) {
                    SWIMMER_LINE_H * 2.0
                } else {
                    SWIMMER_LINE_H
                }
            }
            PrintLine::Gap => EVENT_GAP_H,
        }
    }
}

// One atomic group of lines that must never be split across a column or
// page break: a heat (optionally preceded by its event's name/divider, for
// the first heat of that event) stays together, and a standalone gap
// separates one event's heats from the next event's name.
struct Chunk<'a> {
    lines: Vec<PrintLine<'a>>,
    // The event number this chunk opens with (the first heat's chunk only),
    // so pack_columns can find where to force a page break. None for every
    // other chunk (later heats of the same event, and the trailing gap).
    starts_event: Option<u32>,
}

impl Chunk<'_> {
    fn height(&self) -> f32 {
        self.lines.iter().map(PrintLine::height).sum()
    }
}

fn build_chunks(events: &[PrintEvent], show_entry_times: bool) -> Vec<Chunk<'_>> {
    let mut chunks = Vec::new();
    for event in events {
        for (index, heat) in event.heats.iter().enumerate() {
            let mut lines = Vec::new();
            if index == 0 {
                let (first, second) = wrap_event_name(&event.event_name);
                lines.push(PrintLine::EventName(first, second));
                lines.push(PrintLine::Divider);
                for record in &event.records {
                    lines.push(PrintLine::Record(record));
                }
            }
            lines.push(PrintLine::HeatLabel(&heat.heat_label));
            for swimmer in &heat.swimmers {
                lines.push(PrintLine::Swimmer(
                    swimmer.lane,
                    &swimmer.last_name,
                    &swimmer.first_name,
                    swimmer.age,
                    &swimmer.team,
                    swimmer.exhibition,
                    show_entry_times.then(|| swimmer.seed_time.to_string()),
                ));
            }
            chunks.push(Chunk {
                lines,
                starts_event: (index == 0).then_some(event.number),
            });
        }
        chunks.push(Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: None,
        });
    }
    chunks
}

// Packs chunks into columns, then (if `page_break_before_event` names an
// event present in `chunks`) pads the column list with empty columns so
// that event's opening chunk lands on a fresh page rather than wherever it
// would otherwise fall — the same trick as inserting a manual page break in
// a word processor. A page holds COLUMNS columns, so "a fresh page" means
// the next column index that's a multiple of COLUMNS.
fn pack_columns(
    chunks: Vec<Chunk<'_>>,
    page_break_before_event: Option<u32>,
) -> Vec<Vec<PrintLine<'_>>> {
    let mut columns: Vec<Vec<PrintLine<'_>>> = Vec::new();
    let mut current: Vec<PrintLine<'_>> = Vec::new();
    let mut used = 0.0f32;
    for chunk in chunks {
        if chunk.starts_event.is_some() && chunk.starts_event == page_break_before_event {
            if !current.is_empty() {
                columns.push(std::mem::take(&mut current));
                used = 0.0;
            }
            while !columns.len().is_multiple_of(COLUMNS) {
                columns.push(Vec::new());
            }
        }

        // A lone trailing gap only exists to separate this event from the
        // next one within the same column. If it's what overflows the
        // column, the column break itself is all the separation needed —
        // otherwise it'd carry over as an orphaned blank line at the top of
        // the next column, ahead of that column's first real event name.
        let is_trailing_gap = matches!(chunk.lines.as_slice(), [PrintLine::Gap]);

        let h = chunk.height();
        if used + h > COLUMN_HEIGHT && !current.is_empty() {
            columns.push(std::mem::take(&mut current));
            used = 0.0;
            if is_trailing_gap {
                continue;
            }
        }
        used += h;
        current.extend(chunk.lines);
    }
    if !current.is_empty() {
        columns.push(current);
    }
    columns
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::Rgb(Rgb::new(r, g, b, None))
}

fn show_text_at(ops: &mut Vec<Op>, font: BuiltinFont, size: f32, x: f32, y: f32, text: &str) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: PdfFontHandle::Builtin(font),
        size: Pt(size),
    });
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}

fn draw_hline(ops: &mut Vec<Op>, x_start: f32, x_end: f32, y: f32, thickness: f32, color: Color) {
    ops.push(Op::SetOutlineColor { col: color });
    ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x_start), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x_end), Mm(y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
}

fn draw_vline(ops: &mut Vec<Op>, x: f32, y_start: f32, y_end: f32, thickness: f32, color: Color) {
    ops.push(Op::SetOutlineColor { col: color });
    ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x), Mm(y_start)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x), Mm(y_end)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
}

// printpdf's Rect has no corner-radius option, so a rounded outline has to
// be hand-built as a path: straight edges plus four cubic-bezier corners,
// using the standard kappa constant to approximate a quarter-circle arc.
fn rounded_rect_line(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Line {
    const KAPPA: f32 = 0.552_284_8;
    let r = radius.min(width / 2.0).min(height / 2.0);
    let k = r * (1.0 - KAPPA);

    let pt = |px: f32, py: f32| LinePoint {
        p: Point::new(Mm(px), Mm(py)),
        bezier: false,
    };
    let ctrl = |px: f32, py: f32| LinePoint {
        p: Point::new(Mm(px), Mm(py)),
        bezier: true,
    };

    Line {
        points: vec![
            pt(x + r, y),
            pt(x + width - r, y),
            ctrl(x + width - k, y),
            ctrl(x + width, y + k),
            pt(x + width, y + r),
            pt(x + width, y + height - r),
            ctrl(x + width, y + height - k),
            ctrl(x + width - k, y + height),
            pt(x + width - r, y + height),
            pt(x + r, y + height),
            ctrl(x + k, y + height),
            ctrl(x, y + height - k),
            pt(x, y + height - r),
            pt(x, y + r),
            ctrl(x, y + k),
            ctrl(x + k, y),
            pt(x + r, y),
        ],
        is_closed: true,
    }
}

fn draw_exh_badge(ops: &mut Vec<Op>, x: f32, y: f32) {
    show_text_at(
        ops,
        BuiltinFont::HelveticaBold,
        4.5,
        x + 0.9,
        y + 0.3,
        "EXH",
    );
    ops.push(Op::SetOutlineColor {
        col: rgb(0.35, 0.35, 0.35),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.35) });
    ops.push(Op::DrawLine {
        line: rounded_rect_line(x, y - 0.6, EXH_BOX_W, EXH_BOX_H, EXH_BOX_RADIUS),
    });
}

// Real acronyms range from one letter ("L") to a few ("NVSL"); the box
// widens for anything longer than 2 characters (font size stays fixed) so
// the text never overflows it. The swimmer name then starts right after
// the box instead of at the fixed NAME_X, so it's never crowded either.
fn record_box_width(acronym: &str) -> f32 {
    let len = acronym.chars().count().max(1) as f32;
    RECORD_BOX_W + (len - 2.0).max(0.0) * 1.1
}

// A filled black square holding the record holder's team acronym in white
// bold text, e.g. the "FO" box in front of a pool/meet record's name.
fn draw_record_box(ops: &mut Vec<Op>, x: f32, y: f32, acronym: &str) {
    let box_w = record_box_width(acronym);

    ops.push(Op::SetFillColor {
        col: rgb(0.0, 0.0, 0.0),
    });
    ops.push(Op::DrawRectangle {
        rectangle: Rect {
            x: Mm(x).into(),
            y: Mm(y - 0.6).into(),
            width: Mm(box_w).into(),
            height: Mm(RECORD_BOX_H).into(),
            mode: Some(PaintMode::Fill),
            winding_order: None,
        },
    });
    ops.push(Op::SetFillColor {
        col: rgb(1.0, 1.0, 1.0),
    });
    show_text_at(
        ops,
        BuiltinFont::HelveticaBold,
        RECORD_ACRONYM_SIZE,
        x + 0.6,
        y + 0.2,
        acronym,
    );
    // Every other line in the document relies on the default black fill for
    // text, so it must be restored once this badge is done with white.
    ops.push(Op::SetFillColor {
        col: rgb(0.0, 0.0, 0.0),
    });
}

fn emit_header(
    ops: &mut Vec<Op>,
    left_label: &str,
    meet_title: &str,
    page: usize,
    total_pages: usize,
) {
    show_text_at(
        ops,
        BuiltinFont::HelveticaBold,
        11.0,
        MARGIN,
        HEADER_TEXT_Y,
        left_label,
    );
    show_text_at(
        ops,
        BuiltinFont::HelveticaBold,
        11.0,
        PAGE_W / 2.0 - 30.0,
        HEADER_TEXT_Y,
        meet_title,
    );
    show_text_at(
        ops,
        BuiltinFont::HelveticaBold,
        11.0,
        PAGE_W - MARGIN - 35.0,
        HEADER_TEXT_Y,
        &format!("Page {page} of {total_pages}"),
    );

    draw_hline(
        ops,
        MARGIN,
        PAGE_W - MARGIN,
        HEADER_DIVIDER_Y,
        1.0,
        rgb(0.0, 0.0, 0.0),
    );
}

fn emit_column(ops: &mut Vec<Op>, lines: &[PrintLine<'_>], col_x: f32) {
    let mut y = CONTENT_TOP;
    for line in lines {
        match line {
            PrintLine::EventName(first_line, second_line) => {
                show_text_at(ops, BuiltinFont::HelveticaBold, 8.0, col_x, y, first_line);
                if let Some(second_line) = second_line {
                    show_text_at(
                        ops,
                        BuiltinFont::HelveticaBold,
                        8.0,
                        col_x,
                        y - EVENT_LINE_H,
                        second_line,
                    );
                }
            }
            PrintLine::Divider => {
                draw_hline(ops, col_x, col_x + COL_WIDTH, y, 0.5, rgb(0.5, 0.5, 0.5));
            }
            PrintLine::Record(record) => {
                draw_record_box(ops, col_x + LANE_X, y, &record.team_acronym);
                // A wider box (for a 3-4 character acronym) pushes the name
                // start to the right accordingly, same gap as NAME_X leaves
                // for the standard 1-2 character box.
                let record_name_x = LANE_X + record_box_width(&record.team_acronym) + 0.5;
                show_text_at(
                    ops,
                    BuiltinFont::HelveticaBold,
                    7.0,
                    col_x + record_name_x,
                    y,
                    &record.swimmer_name,
                );
                // A long name gets its own line, pushing year/time down —
                // otherwise they'd run into the tail end of the name.
                let rest_y = if record.swimmer_name.chars().count() > RECORD_NAME_WRAP_THRESHOLD {
                    y - RECORD_LINE_H
                } else {
                    y
                };
                let year_text = record.year.to_string();
                show_text_at(
                    ops,
                    BuiltinFont::HelveticaBold,
                    7.0,
                    col_x + AGE_RIGHT_X - digits_width_mm(&year_text, 7.0),
                    rest_y,
                    &year_text,
                );
                show_text_at(
                    ops,
                    BuiltinFont::HelveticaBold,
                    7.0,
                    col_x + TIME_X,
                    rest_y,
                    &record.time,
                );
            }
            PrintLine::HeatLabel(label) => {
                show_text_at(ops, BuiltinFont::HelveticaOblique, 7.0, col_x, y, label);
            }
            PrintLine::Swimmer(lane, last, first, age, team, exhibition, entry_time) => {
                show_text_at(
                    ops,
                    BuiltinFont::Helvetica,
                    7.0,
                    col_x + LANE_X,
                    y,
                    &lane.to_string(),
                );

                // Long names push the first name to a second line so the
                // EXH badge always has room next to whatever's on that line.
                let rest_y = if full_name_len(last, first) > name_wrap_threshold(*exhibition) {
                    show_text_at(
                        ops,
                        BuiltinFont::Helvetica,
                        7.0,
                        col_x + NAME_X,
                        y,
                        &format!("{last},"),
                    );
                    let second_line_y = y - SWIMMER_LINE_H;
                    show_text_at(
                        ops,
                        BuiltinFont::Helvetica,
                        7.0,
                        col_x + NAME_X,
                        second_line_y,
                        first,
                    );
                    second_line_y
                } else {
                    show_text_at(
                        ops,
                        BuiltinFont::Helvetica,
                        7.0,
                        col_x + NAME_X,
                        y,
                        &format!("{last}, {first}"),
                    );
                    y
                };

                if *exhibition {
                    draw_exh_badge(ops, col_x + EXH_X, rest_y);
                }
                let age_text = age.to_string();
                show_text_at(
                    ops,
                    BuiltinFont::Helvetica,
                    7.0,
                    col_x + AGE_RIGHT_X - digits_width_mm(&age_text, 7.0),
                    rest_y,
                    &age_text,
                );
                show_text_at(
                    ops,
                    BuiltinFont::Helvetica,
                    7.0,
                    col_x + TEAM_X,
                    rest_y,
                    team,
                );
                match entry_time {
                    Some(entry_time) => {
                        show_text_at(
                            ops,
                            BuiltinFont::Helvetica,
                            7.0,
                            col_x + TIME_X,
                            rest_y,
                            entry_time,
                        );
                    }
                    None => {
                        draw_hline(
                            ops,
                            col_x + TIME_X,
                            col_x + COL_WIDTH,
                            rest_y - 0.5,
                            0.5,
                            rgb(0.0, 0.0, 0.0),
                        );
                    }
                }
            }
            PrintLine::Gap => {}
        }
        y -= line.height();
    }
}

pub fn write_pdf(
    meet_title: &str,
    events: &[PrintEvent],
    show_entry_times: bool,
    page_break_before_event: Option<u32>,
    path: &Path,
) -> Result<(), String> {
    let chunks = build_chunks(events, show_entry_times);
    let columns = pack_columns(chunks, page_break_before_event);
    let pages: Vec<&[Vec<PrintLine<'_>>]> = if columns.is_empty() {
        vec![&[]]
    } else {
        columns.chunks(COLUMNS).collect()
    };
    let total_pages = pages.len();

    let mut doc = PdfDocument::new(meet_title);
    let mut pdf_pages = Vec::new();
    for (page_index, page_columns) in pages.iter().enumerate() {
        let mut ops = Vec::new();
        emit_header(
            &mut ops,
            "Heat Sheet",
            meet_title,
            page_index + 1,
            total_pages,
        );
        for (col_index, column_lines) in page_columns.iter().enumerate() {
            let col_x = MARGIN + col_index as f32 * (COL_WIDTH + GUTTER);
            emit_column(&mut ops, column_lines, col_x);
        }
        for col_index in 0..COLUMNS - 1 {
            let divider_x =
                MARGIN + col_index as f32 * (COL_WIDTH + GUTTER) + COL_WIDTH + GUTTER / 2.0;
            draw_vline(
                &mut ops,
                divider_x,
                CONTENT_TOP,
                MARGIN,
                0.3,
                rgb(0.7, 0.7, 0.7),
            );
        }
        pdf_pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
    }
    doc.with_pages(pdf_pages);

    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "export/tests.rs"]
mod tests;
