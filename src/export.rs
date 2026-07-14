use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use printpdf::*;

use crate::merge::{MixedHeat, MixedHeatSource};
use crate::model::{Event, Lane, Meet};

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
const AGE_X: f32 = 32.0;
const TEAM_X: f32 = 38.0;
const TIME_X: f32 = 52.0;

// "Last, First" longer than this wraps the first name onto its own line so
// the EXH badge has room next to whatever's left on the name's line.
const NAME_WRAP_THRESHOLD: usize = 15;

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
}

fn abbreviate<'a>(team: &'a str, abbreviations: &'a HashMap<String, String>) -> &'a str {
    abbreviations
        .get(team)
        .map(String::as_str)
        .filter(|a| !a.is_empty())
        .unwrap_or(team)
}

fn swimmer_rows(
    lanes: &[Lane],
    abbreviations: &HashMap<String, String>,
) -> Vec<PrintSwimmer> {
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

// "Heats 1 of 2 and 1 of 1" — each source's original heat number/total,
// ordered by event number rather than merge order.
fn mixed_heat_label(meet: &Meet, sources: &[MixedHeatSource]) -> String {
    let mut sorted: Vec<&MixedHeatSource> = sources.iter().collect();
    sorted.sort_by_key(|s| s.event_number);

    let parts: Vec<String> = sorted
        .iter()
        .filter_map(|s| {
            meet.events
                .iter()
                .find(|e| e.number == s.event_number)
                .and_then(|e| e.heats.iter().find(|h| h.number == s.heat_number))
                .map(|h| format!("{} of {}", h.number, h.of))
        })
        .collect();

    let joined = match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {}", rest.join(", "), last),
        _ => parts.join(", "),
    };
    format!("Heats {joined}")
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

// Walks events in rotated print order; for each event, emits one PrintEvent
// holding every remaining (non-consumed) heat, then interleaves any mixed
// heats anchored to that event number, mirroring the GUI's Final Preview
// ordering. Skips events left with no remaining heats.
pub fn build_print_events(
    meet: &Meet,
    consumed: &HashSet<(u32, u32)>,
    mixed_heats: &[MixedHeat],
    abbreviations: &HashMap<String, String>,
    start_event: u32,
) -> Vec<PrintEvent> {
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
            });
        }

        for mixed in mixed_heats {
            if mixed.anchor_event() == event.number {
                events.push(PrintEvent {
                    event_name: mixed.header.clone(),
                    heats: vec![PrintHeat {
                        heat_label: mixed_heat_label(meet, &mixed.sources),
                        swimmers: swimmer_rows(&mixed.lanes, abbreviations),
                    }],
                });
            }
        }
    }
    events
}

enum PrintLine<'a> {
    EventName(&'a str),
    Divider,
    HeatLabel(&'a str),
    // (lane, last_name, first_name, age, team, exhibition)
    Swimmer(u32, &'a str, &'a str, u32, &'a str, bool),
    Gap,
}

impl PrintLine<'_> {
    fn height(&self) -> f32 {
        match self {
            PrintLine::EventName(_) => EVENT_LINE_H,
            PrintLine::Divider => DIVIDER_LINE_H,
            PrintLine::HeatLabel(_) => HEAT_LABEL_LINE_H,
            PrintLine::Swimmer(_, last, first, ..) => {
                if full_name_len(last, first) > NAME_WRAP_THRESHOLD {
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
}

impl Chunk<'_> {
    fn height(&self) -> f32 {
        self.lines.iter().map(PrintLine::height).sum()
    }
}

fn build_chunks(events: &[PrintEvent]) -> Vec<Chunk<'_>> {
    let mut chunks = Vec::new();
    for event in events {
        for (index, heat) in event.heats.iter().enumerate() {
            let mut lines = Vec::new();
            if index == 0 {
                lines.push(PrintLine::EventName(&event.event_name));
                lines.push(PrintLine::Divider);
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
                ));
            }
            chunks.push(Chunk { lines });
        }
        chunks.push(Chunk {
            lines: vec![PrintLine::Gap],
        });
    }
    chunks
}

fn pack_columns(chunks: Vec<Chunk<'_>>) -> Vec<Vec<PrintLine<'_>>> {
    let mut columns: Vec<Vec<PrintLine<'_>>> = Vec::new();
    let mut current: Vec<PrintLine<'_>> = Vec::new();
    let mut used = 0.0f32;
    for chunk in chunks {
        let h = chunk.height();
        if used + h > COLUMN_HEIGHT && !current.is_empty() {
            columns.push(std::mem::take(&mut current));
            used = 0.0;
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
    show_text_at(ops, BuiltinFont::HelveticaBold, 4.5, x + 0.9, y + 0.3, "EXH");
    ops.push(Op::SetOutlineColor {
        col: rgb(0.35, 0.35, 0.35),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.35) });
    ops.push(Op::DrawLine {
        line: rounded_rect_line(x, y - 0.6, EXH_BOX_W, EXH_BOX_H, EXH_BOX_RADIUS),
    });
}

fn emit_header(ops: &mut Vec<Op>, meet_title: &str, page: usize, total_pages: usize) {
    show_text_at(
        ops,
        BuiltinFont::HelveticaBold,
        11.0,
        MARGIN,
        HEADER_TEXT_Y,
        "Heat Sheet",
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
            PrintLine::EventName(name) => {
                show_text_at(ops, BuiltinFont::HelveticaBold, 8.0, col_x, y, name);
            }
            PrintLine::Divider => {
                draw_hline(
                    ops,
                    col_x,
                    col_x + COL_WIDTH,
                    y,
                    0.5,
                    rgb(0.5, 0.5, 0.5),
                );
            }
            PrintLine::HeatLabel(label) => {
                show_text_at(ops, BuiltinFont::HelveticaOblique, 7.0, col_x, y, label);
            }
            PrintLine::Swimmer(lane, last, first, age, team, exhibition) => {
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
                let rest_y = if full_name_len(last, first) > NAME_WRAP_THRESHOLD {
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
                show_text_at(
                    ops,
                    BuiltinFont::Helvetica,
                    7.0,
                    col_x + AGE_X,
                    rest_y,
                    &age.to_string(),
                );
                show_text_at(ops, BuiltinFont::Helvetica, 7.0, col_x + TEAM_X, rest_y, team);
                draw_hline(
                    ops,
                    col_x + TIME_X,
                    col_x + COL_WIDTH,
                    rest_y - 0.5,
                    0.5,
                    rgb(0.0, 0.0, 0.0),
                );
            }
            PrintLine::Gap => {}
        }
        y -= line.height();
    }
}

pub fn write_pdf(meet_title: &str, events: &[PrintEvent], path: &Path) -> Result<(), String> {
    let chunks = build_chunks(events);
    let columns = pack_columns(chunks);
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
        emit_header(&mut ops, meet_title, page_index + 1, total_pages);
        for (col_index, column_lines) in page_columns.iter().enumerate() {
            let col_x = MARGIN + col_index as f32 * (COL_WIDTH + GUTTER);
            emit_column(&mut ops, column_lines, col_x);
        }
        pdf_pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
    }
    doc.with_pages(pdf_pages);

    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Heat, SeedTime, Swimmer};

    fn event(number: u32, heats: Vec<Heat>) -> Event {
        Event {
            number,
            gender: "Boys".to_string(),
            age_group: "10-11".to_string(),
            distance_m: 25,
            stroke: "Freestyle".to_string(),
            heats,
        }
    }

    fn swimmer(name: &str) -> Swimmer {
        Swimmer {
            last_name: name.to_string(),
            first_name: "Test".to_string(),
            age: 10,
            exhibition: false,
            team: "TST".to_string(),
            seed_time: SeedTime::Seconds(20.0),
        }
    }

    fn heat_with_lanes(number: u32, of: u32, lane_count: u32) -> Heat {
        Heat {
            number,
            of,
            lanes: (1..=lane_count)
                .map(|n| Lane {
                    number: n,
                    swimmer: Some(swimmer("Doe")),
                })
                .collect(),
        }
    }

    fn heat(number: u32, of: u32) -> Heat {
        heat_with_lanes(number, of, 1)
    }

    fn no_abbreviations() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn rotate_events_default_start_is_a_no_op() {
        let events = vec![event(1, vec![]), event(2, vec![]), event(3, vec![])];
        let rotated: Vec<u32> = rotate_events(&events, 1).iter().map(|e| e.number).collect();
        assert_eq!(rotated, vec![1, 2, 3]);
    }

    #[test]
    fn rotate_events_puts_start_and_above_first() {
        let events: Vec<Event> = (1..=5).map(|n| event(n, vec![])).collect();
        let rotated: Vec<u32> = rotate_events(&events, 3).iter().map(|e| e.number).collect();
        assert_eq!(rotated, vec![3, 4, 5, 1, 2]);
    }

    #[test]
    fn rotate_events_beyond_max_is_a_no_op() {
        let events = vec![event(1, vec![]), event(2, vec![])];
        let rotated: Vec<u32> = rotate_events(&events, 99).iter().map(|e| e.number).collect();
        assert_eq!(rotated, vec![1, 2]);
    }

    #[test]
    fn build_print_events_groups_heats_under_one_event_name() {
        let meet = Meet {
            title: "Test Meet".to_string(),
            date: "Jan 1".to_string(),
            events: vec![event(1, vec![heat(1, 2), heat(2, 2)])],
        };
        let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].heats.len(), 2);
        assert_eq!(events[0].heats[0].heat_label, "Heat 1 of 2");
        assert_eq!(events[0].heats[1].heat_label, "Heat 2 of 2");
    }

    #[test]
    fn build_print_events_skips_consumed_heats_and_empty_events() {
        let meet = Meet {
            title: "Test Meet".to_string(),
            date: "Jan 1".to_string(),
            events: vec![event(1, vec![heat(1, 2), heat(2, 2)]), event(2, vec![heat(1, 1)])],
        };
        let mut consumed = HashSet::new();
        consumed.insert((1, 1));
        consumed.insert((2, 1));

        let events = build_print_events(&meet, &consumed, &[], &no_abbreviations(), 1);
        // Event 2 has no remaining heats and should be dropped entirely.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].heats.len(), 1);
        assert_eq!(events[0].heats[0].heat_label, "Heat 2 of 2");
    }

    #[test]
    fn build_print_events_interleaves_mixed_heat_at_anchor_event() {
        let meet = Meet {
            title: "Test Meet".to_string(),
            date: "Jan 1".to_string(),
            events: vec![event(1, vec![heat(1, 1)]), event(2, vec![heat(1, 1)])],
        };
        let mut consumed = HashSet::new();
        consumed.insert((1, 1));
        consumed.insert((2, 1));

        let mixed = MixedHeat {
            header: "#1/2 25m Freestyle".to_string(),
            sources: vec![
                MixedHeatSource {
                    event_number: 1,
                    heat_number: 1,
                    gender: "Boys".to_string(),
                    distance_m: 25,
                    stroke: "Freestyle".to_string(),
                    age_group: "10-11".to_string(),
                },
                MixedHeatSource {
                    event_number: 2,
                    heat_number: 1,
                    gender: "Boys".to_string(),
                    distance_m: 25,
                    stroke: "Freestyle".to_string(),
                    age_group: "10-11".to_string(),
                },
            ],
            lanes: vec![],
        };

        let events = build_print_events(&meet, &consumed, &[mixed], &no_abbreviations(), 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "#1/2 25m Freestyle");
        assert_eq!(events[0].heats[0].heat_label, "Heats 1 of 1 and 1 of 1");
    }

    #[test]
    fn mixed_heat_label_orders_by_event_number_regardless_of_source_order() {
        let meet = Meet {
            title: "Test Meet".to_string(),
            date: "Jan 1".to_string(),
            events: vec![
                event(1, vec![heat(2, 3)]),
                event(5, vec![heat(1, 4)]),
            ],
        };
        // Sources given out of event-number order on purpose.
        let sources = vec![
            MixedHeatSource {
                event_number: 5,
                heat_number: 1,
                gender: "Boys".to_string(),
                distance_m: 25,
                stroke: "Freestyle".to_string(),
                age_group: "10-11".to_string(),
            },
            MixedHeatSource {
                event_number: 1,
                heat_number: 2,
                gender: "Boys".to_string(),
                distance_m: 25,
                stroke: "Freestyle".to_string(),
                age_group: "10-11".to_string(),
            },
        ];
        assert_eq!(
            mixed_heat_label(&meet, &sources),
            "Heats 2 of 3 and 1 of 4"
        );
    }

    #[test]
    fn distinct_teams_deduplicates_and_sorts() {
        let meet = Meet {
            title: "Test Meet".to_string(),
            date: "Jan 1".to_string(),
            events: vec![event(
                1,
                vec![Heat {
                    number: 1,
                    of: 1,
                    lanes: vec![
                        Lane {
                            number: 1,
                            swimmer: Some(Swimmer {
                                team: "Zeta".to_string(),
                                ..swimmer("A")
                            }),
                        },
                        Lane {
                            number: 2,
                            swimmer: Some(Swimmer {
                                team: "Alpha".to_string(),
                                ..swimmer("B")
                            }),
                        },
                        Lane {
                            number: 3,
                            swimmer: Some(Swimmer {
                                team: "Alpha".to_string(),
                                ..swimmer("C")
                            }),
                        },
                    ],
                }],
            )],
        };
        let teams = distinct_teams(&meet, &HashSet::new(), &[]);
        assert_eq!(teams, vec!["Alpha".to_string(), "Zeta".to_string()]);
    }

    #[test]
    fn abbreviations_are_applied_to_swimmer_rows() {
        let mut abbreviations = HashMap::new();
        abbreviations.insert("Fair Oaks Sharks".to_string(), "FOS".to_string());
        let lanes = vec![Lane {
            number: 1,
            swimmer: Some(Swimmer {
                team: "Fair Oaks Sharks".to_string(),
                ..swimmer("Doe")
            }),
        }];
        let rows = swimmer_rows(&lanes, &abbreviations);
        assert_eq!(rows[0].team, "FOS");
    }

    #[test]
    fn swimmer_rows_carries_exhibition_flag() {
        let lanes = vec![
            Lane {
                number: 1,
                swimmer: Some(Swimmer {
                    exhibition: true,
                    ..swimmer("Doe")
                }),
            },
            Lane {
                number: 2,
                swimmer: Some(swimmer("Smith")),
            },
        ];
        let rows = swimmer_rows(&lanes, &HashMap::new());
        assert!(rows[0].exhibition);
        assert!(!rows[1].exhibition);
    }

    #[test]
    fn long_names_take_two_lines_short_names_take_one() {
        let short = PrintLine::Swimmer(1, "Doe", "Jo", 10, "TST", false);
        assert_eq!(short.height(), SWIMMER_LINE_H);

        let long = PrintLine::Swimmer(1, "Featherstonehaugh", "Jonathan", 10, "TST", false);
        assert_eq!(long.height(), SWIMMER_LINE_H * 2.0);
    }

    #[test]
    fn a_heat_never_splits_across_columns() {
        // One heat with enough swimmers to fill more than a full column by
        // itself; it must all land in one column, so the *next* heat should
        // start a fresh column rather than continuing mid-heat.
        let meet = Meet {
            title: "Test Meet".to_string(),
            date: "Jan 1".to_string(),
            events: vec![event(1, vec![heat_with_lanes(1, 2, 60), heat_with_lanes(2, 2, 2)])],
        };
        let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1);
        let chunks = build_chunks(&events);
        let columns = pack_columns(chunks);

        // Count how many "Heat 2 of 2" heat-label lines land in each column;
        // it must be fully contained in exactly one column, not split.
        for column in &columns {
            let heat2_lines = column
                .iter()
                .filter(|l| matches!(l, PrintLine::HeatLabel(s) if *s == "Heat 2 of 2"))
                .count();
            assert!(heat2_lines <= 1);
        }
    }

    #[test]
    fn write_pdf_produces_a_valid_pdf_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("meetmerger_export_test.pdf");
        let print_event = PrintEvent {
            event_name: "#1 Boys 10-11 25m Freestyle".to_string(),
            heats: vec![PrintHeat {
                heat_label: "Heat 1 of 1".to_string(),
                swimmers: vec![PrintSwimmer {
                    lane: 1,
                    last_name: "Doe".to_string(),
                    first_name: "Jane".to_string(),
                    age: 10,
                    team: "TST".to_string(),
                    exhibition: false,
                }],
            }],
        };
        write_pdf("Test Meet", &[print_event], &path).expect("write_pdf should succeed");

        let bytes = std::fs::read(&path).expect("file should exist");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 100);
        std::fs::remove_file(&path).ok();
    }

    // Manual verification against the real (gitignored) sample heat sheet.
    // Run with: cargo test --lib export::tests::manual_export_sample_heat_sheet -- --ignored
    #[test]
    #[ignore]
    fn manual_export_sample_heat_sheet() {
        let pdf_path = std::path::Path::new("test-data/sample_heat_sheet.pdf");
        if !pdf_path.exists() {
            return;
        }

        let raw = pdf_extract::extract_text(pdf_path).expect("extract_text should succeed");
        let corrections_path = pdf_path.with_extension("corrections.txt");
        let corrections = if corrections_path.exists() {
            std::fs::read_to_string(&corrections_path)
                .unwrap()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .filter_map(|line| line.split_once('='))
                .map(|(f, r)| (f.to_string(), r.to_string()))
                .collect()
        } else {
            Vec::new()
        };
        let text = crate::parse::apply_corrections(
            &crate::parse::normalize_corruption(&raw),
            &corrections,
        );
        let (meet, issues) = crate::parse::parse_meet(&text);
        assert!(issues.is_empty(), "unexpected parse issues: {issues:?}");

        let abbreviations = HashMap::new();
        let events = build_print_events(&meet, &HashSet::new(), &[], &abbreviations, 1);
        assert_eq!(events.len(), meet.events.len());

        let out_path = std::env::temp_dir().join("meetmerger_sample_export.pdf");
        write_pdf(&meet.title, &events, &out_path).expect("write_pdf should succeed");
        let bytes = std::fs::read(&out_path).expect("file should exist");
        assert!(bytes.starts_with(b"%PDF-"));
        println!(
            "wrote {} bytes, {} events, to {}",
            bytes.len(),
            events.len(),
            out_path.display()
        );

        let max_event = meet.events.iter().map(|e| e.number).max().unwrap_or(1);
        let rotated_events = build_print_events(&meet, &HashSet::new(), &[], &abbreviations, max_event);
        assert_eq!(rotated_events[0].event_name, events.last().unwrap().event_name);
    }
}
