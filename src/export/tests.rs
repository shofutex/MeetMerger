use super::*;
use crate::merge::MixedHeatSource;
use crate::model::{Heat, Record, SeedTime, Swimmer};

fn event(number: u32, heats: Vec<Heat>) -> Event {
    Event {
        number,
        gender: "Boys".to_string(),
        age_group: "10-11".to_string(),
        distance_m: 25,
        stroke: "Freestyle".to_string(),
        heats,
        records: Vec::new(),
    }
}

fn event_with_records(number: u32, heats: Vec<Heat>, records: Vec<Record>) -> Event {
    Event {
        records,
        ..event(number, heats)
    }
}

fn fair_oaks_record() -> Record {
    Record {
        team_acronym: "FO".to_string(),
        swimmer_name: "Anthony Grimm".to_string(),
        year: 2011,
        time: "18.16".to_string(),
        team_name: "Fair Oaks Sharks".to_string(),
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
    let rotated: Vec<u32> = rotate_events(&events, 99)
        .iter()
        .map(|e| e.number)
        .collect();
    assert_eq!(rotated, vec![1, 2]);
}

#[test]
fn build_print_events_groups_heats_under_one_event_name() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event(1, vec![heat(1, 2), heat(2, 2)])],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

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
        events: vec![
            event(1, vec![heat(1, 2), heat(2, 2)]),
            event(2, vec![heat(1, 1)]),
        ],
    };
    let mut consumed = HashSet::new();
    consumed.insert((1, 1));
    consumed.insert((2, 1));

    let events = build_print_events(&meet, &consumed, &[], &no_abbreviations(), 1, false);

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
        // heat_index/heat_count 2 of 3, so the heat_label assertion below
        // distinguishes the split-based label from the old
        // original-source-heat-based one.
        heat_index: 2,
        heat_count: 3,
    };

    let events = build_print_events(&meet, &consumed, &[mixed], &no_abbreviations(), 1, false);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_name, "#1/2 25m Freestyle");
    assert_eq!(events[0].heats[0].heat_label, "Heat 2 of 3");
}

#[test]
fn build_print_events_attaches_records_when_show_records_is_true() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event_with_records(
            1,
            vec![heat(1, 1)],
            vec![fair_oaks_record()],
        )],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, true);

    assert_eq!(events[0].records.len(), 1);
    let record = &events[0].records[0];
    assert_eq!(record.team_acronym, "FO");
    assert_eq!(record.swimmer_name, "Anthony Grimm");
    assert_eq!(record.year, 2011);
    assert_eq!(record.time, "18.16");
}

#[test]
fn build_print_events_attaches_multiple_teams_records() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event_with_records(
            1,
            vec![heat(1, 1)],
            vec![
                fair_oaks_record(),
                Record {
                    team_acronym: "WC".to_string(),
                    swimmer_name: "Nathaniel Temeles".to_string(),
                    year: 2015,
                    time: "19.67".to_string(),
                    team_name: String::new(),
                },
            ],
        )],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, true);

    assert_eq!(events[0].records.len(), 2);
    assert_eq!(events[0].records[0].team_acronym, "FO");
    assert_eq!(events[0].records[1].team_acronym, "WC");
}

#[test]
fn build_print_events_omits_records_when_show_records_is_false() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event_with_records(
            1,
            vec![heat(1, 1)],
            vec![fair_oaks_record()],
        )],
    };
    let events =
        build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

    assert!(events[0].records.is_empty());
}

#[test]
fn build_print_events_never_attaches_records_to_a_mixed_heat_group() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![
            event_with_records(1, vec![heat(1, 1)], vec![fair_oaks_record()]),
            event(2, vec![heat(1, 1)]),
        ],
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
        heat_index: 1,
        heat_count: 1,
    };

    let events = build_print_events(&meet, &consumed, &[mixed], &no_abbreviations(), 1, true);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_name, "#1/2 25m Freestyle");
    assert!(events[0].records.is_empty());
}

fn mixed_source(event_number: u32, heat_number: u32) -> MixedHeatSource {
    MixedHeatSource {
        event_number,
        heat_number,
        gender: "Boys".to_string(),
        distance_m: 25,
        stroke: "Freestyle".to_string(),
        age_group: "10-11".to_string(),
    }
}

#[test]
fn build_print_events_groups_a_mixed_heats_splits_under_one_event() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event(1, vec![heat(1, 1)]), event(2, vec![heat(1, 1)])],
    };
    let mut consumed = HashSet::new();
    consumed.insert((1, 1));
    consumed.insert((2, 1));

    let sources = vec![mixed_source(1, 1), mixed_source(2, 1)];
    let splits: Vec<MixedHeat> = (1..=3)
        .map(|heat_index| MixedHeat {
            header: "#1/2 25m Freestyle".to_string(),
            sources: sources.clone(),
            lanes: vec![],
            heat_index,
            heat_count: 3,
        })
        .collect();

    let events = build_print_events(&meet, &consumed, &splits, &no_abbreviations(), 1, false);

    // One event name, shown once, not repeated per split.
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_name, "#1/2 25m Freestyle");
    assert_eq!(events[0].heats.len(), 3);
    assert_eq!(events[0].heats[0].heat_label, "Heat 1 of 3");
    assert_eq!(events[0].heats[1].heat_label, "Heat 2 of 3");
    assert_eq!(events[0].heats[2].heat_label, "Heat 3 of 3");
}

#[test]
fn build_changes_finds_each_swimmers_original_lane() {
    let heat_a = Heat {
        number: 1,
        of: 1,
        lanes: vec![
            Lane {
                number: 1,
                swimmer: Some(swimmer("Slow")),
            },
            Lane {
                number: 2,
                swimmer: Some(swimmer("Fast")),
            },
        ],
    };
    let heat_b = Heat {
        number: 1,
        of: 1,
        lanes: vec![Lane {
            number: 1,
            swimmer: Some(swimmer("Mid")),
        }],
    };
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![
            event(1, vec![heat_a.clone()]),
            event(2, vec![heat_b.clone()]),
        ],
    };

    let sources = vec![(mixed_source(1, 1), &heat_a), (mixed_source(2, 1), &heat_b)];
    let mixed = crate::merge::build_mixed_heats(sources, 6);

    let changes = build_changes(&meet, &mixed);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].heats.len(), 1);
    let rows = &changes[0].heats[0].rows;
    assert_eq!(rows.len(), 3);

    let row_for = |name: &str| {
        rows.iter()
            .find(|r| r.swimmer_name.starts_with(name))
            .unwrap()
    };
    let slow = row_for("Slow");
    assert_eq!(
        (slow.original_event, slow.original_heat, slow.original_lane),
        (1, 1, 1)
    );
    let fast = row_for("Fast");
    assert_eq!(
        (fast.original_event, fast.original_heat, fast.original_lane),
        (1, 1, 2)
    );
    let mid = row_for("Mid");
    assert_eq!(
        (mid.original_event, mid.original_heat, mid.original_lane),
        (2, 1, 1)
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
    let short = PrintLine::Swimmer(1, "Doe", "Jo", 10, "TST", false, None);
    assert_eq!(short.height(), SWIMMER_LINE_H);

    let long = PrintLine::Swimmer(1, "Featherstonehaugh", "Jonathan", 10, "TST", false, None);
    assert_eq!(long.height(), SWIMMER_LINE_H * 2.0);
}

fn print_event_with_seed_time(seed_time: SeedTime) -> PrintEvent {
    PrintEvent {
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
                seed_time,
            }],
        }],
        records: Vec::new(),
        number: 1,
    }
}

#[test]
fn build_chunks_omits_entry_time_text_when_show_entry_times_is_false() {
    let events = vec![print_event_with_seed_time(SeedTime::Seconds(20.0))];
    let chunks = build_chunks(&events, false);
    let swimmer_line = chunks[0]
        .lines
        .iter()
        .find(|line| matches!(line, PrintLine::Swimmer(..)))
        .unwrap();
    let PrintLine::Swimmer(.., entry_time) = swimmer_line else {
        unreachable!()
    };
    assert_eq!(entry_time, &None);
}

#[test]
fn build_chunks_includes_formatted_entry_time_when_show_entry_times_is_true() {
    let events = vec![print_event_with_seed_time(SeedTime::Seconds(65.43))];
    let chunks = build_chunks(&events, true);
    let swimmer_line = chunks[0]
        .lines
        .iter()
        .find(|line| matches!(line, PrintLine::Swimmer(..)))
        .unwrap();
    let PrintLine::Swimmer(.., entry_time) = swimmer_line else {
        unreachable!()
    };
    assert_eq!(entry_time.as_deref(), Some("1:05.43"));
}

#[test]
fn build_chunks_formats_no_time_entries_as_nt() {
    let events = vec![print_event_with_seed_time(SeedTime::NoTime)];
    let chunks = build_chunks(&events, true);
    let swimmer_line = chunks[0]
        .lines
        .iter()
        .find(|line| matches!(line, PrintLine::Swimmer(..)))
        .unwrap();
    let PrintLine::Swimmer(.., entry_time) = swimmer_line else {
        unreachable!()
    };
    assert_eq!(entry_time.as_deref(), Some("NT"));
}

#[test]
fn a_heat_never_splits_across_columns() {
    // One heat with enough swimmers to fill more than a full column by
    // itself; it must all land in one column, so the *next* heat should
    // start a fresh column rather than continuing mid-heat.
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event(
            1,
            vec![heat_with_lanes(1, 2, 60), heat_with_lanes(2, 2, 2)],
        )],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

    let chunks = build_chunks(&events, false);
    let columns = pack_columns(chunks, None);

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
fn pack_columns_pads_to_a_fresh_page_before_the_chosen_event() {
    // A chunk sized to fill exactly one column by itself, so the next
    // chunk naturally starts a fresh column even without a forced break.
    let filler_lines = (COLUMN_HEIGHT / EVENT_GAP_H).ceil() as usize;
    let full_column_chunk = Chunk {
        lines: (0..filler_lines).map(|_| PrintLine::Gap).collect(),
        starts_event: Some(1),
    };
    let small_chunk = |n: u32| Chunk {
        lines: vec![PrintLine::Gap],
        starts_event: Some(n),
    };

    let chunks = vec![full_column_chunk, small_chunk(2), small_chunk(3)];
    let columns = pack_columns(chunks, Some(2));

    // Event 1 fills column 0 alone. Event 2 must be pushed to the next
    // page (the next column index that's a multiple of COLUMNS), so
    // columns 1 and 2 are blank padding; event 3 continues right after
    // event 2, since both are tiny enough to share that column.
    assert_eq!(columns.len(), 4);
    assert!(columns[1].is_empty());
    assert!(columns[2].is_empty());
    assert_eq!(columns[3].len(), 2);
}

#[test]
fn pack_columns_ignores_a_page_break_event_that_never_appears() {
    let chunks = vec![
        Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: Some(1),
        },
        Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: Some(2),
        },
    ];
    let with_break = pack_columns(chunks, Some(99));

    let chunks = vec![
        Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: Some(1),
        },
        Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: Some(2),
        },
    ];
    let without_break = pack_columns(chunks, None);

    assert_eq!(with_break.len(), without_break.len());
}

#[test]
fn pack_columns_breaking_before_the_first_event_wastes_no_leading_page() {
    let chunks = vec![
        Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: Some(1),
        },
        Chunk {
            lines: vec![PrintLine::Gap],
            starts_event: Some(2),
        },
    ];
    let columns = pack_columns(chunks, Some(1));

    // Breaking before the very first chunk shouldn't insert a blank
    // leading page -- there's nothing before it to push off the page.
    assert_eq!(columns.len(), 1);
}

#[test]
fn pack_columns_drops_a_trailing_gap_that_would_orphan_at_the_top_of_the_next_column() {
    // Fill the column almost exactly, so the event's *trailing* gap chunk
    // (not its heats) is what overflows -- reproducing a real bug where
    // that gap carried over as a blank line at the very top of the next
    // column, ahead of the next event's name.
    let filler_lines = (COLUMN_HEIGHT / EVENT_GAP_H).floor() as usize;
    let event_1_heats = Chunk {
        lines: (0..filler_lines).map(|_| PrintLine::Gap).collect(),
        starts_event: Some(1),
    };
    let trailing_gap = Chunk {
        lines: vec![PrintLine::Gap],
        starts_event: None,
    };
    let event_2 = Chunk {
        lines: vec![PrintLine::Gap],
        starts_event: Some(2),
    };

    let columns = pack_columns(vec![event_1_heats, trailing_gap, event_2], None);

    assert_eq!(columns.len(), 2);
    assert_eq!(
        columns[1].len(),
        1,
        "the orphaned trailing gap should be dropped, not carried into the next column"
    );
}

#[test]
fn build_timer_pages_lists_every_event_and_marks_empty_lanes() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![
            event(1, vec![heat_with_lanes(1, 1, 2)]),
            event(2, vec![heat_with_lanes(1, 1, 1)]),
        ],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

    let pages = build_timer_pages(&events, 2);

    // Lane 1 swims in both events; lane 2 only swims in event 1, but
    // event 2 still appears on lane 2's page with a "no swimmer" row.
    let lane1 = pages.iter().find(|p| p.lane == 1).unwrap();
    assert_eq!(lane1.events.len(), 2);
    assert!(lane1.events[0].rows[0].swimmer.is_some());

    let lane2 = pages.iter().find(|p| p.lane == 2).unwrap();
    assert_eq!(lane2.events.len(), 2);
    assert!(lane2.events[0].rows[0].swimmer.is_some());
    assert!(lane2.events[1].rows[0].swimmer.is_none());
}

#[test]
fn build_timer_pages_carries_heat_label_and_swimmer_details() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event(
            1,
            vec![heat_with_lanes(1, 2, 1), heat_with_lanes(2, 2, 1)],
        )],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

    let pages = build_timer_pages(&events, 1);

    let lane1 = &pages[0];
    assert_eq!(lane1.events[0].rows.len(), 2);
    assert_eq!(lane1.events[0].rows[0].heat_label, "Heat 1 of 2");
    let swimmer = lane1.events[0].rows[0].swimmer.as_ref().unwrap();
    assert_eq!(swimmer.last_name, "Doe");
    assert_eq!(swimmer.team, "TST");
}

#[test]
fn pack_timer_pages_caps_heats_per_page() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event(
            1,
            vec![
                heat_with_lanes(1, 3, 1),
                heat_with_lanes(2, 3, 1),
                heat_with_lanes(3, 3, 1),
            ],
        )],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

    let pages = build_timer_pages(&events, 1);

    let packed = pack_timer_pages(&pages[0].events, Some(2));
    assert_eq!(packed.len(), 2);
    let count = |lines: &[TimerLine<'_>]| {
        lines
            .iter()
            .filter(|l| matches!(l, TimerLine::Row(..)))
            .count()
    };
    assert_eq!(count(&packed[0]), 2);
    assert_eq!(count(&packed[1]), 1);
}

#[test]
fn pack_timer_pages_repeats_event_header_after_a_break_mid_event() {
    let meet = Meet {
        title: "Test Meet".to_string(),
        date: "Jan 1".to_string(),
        events: vec![event(
            1,
            vec![
                heat_with_lanes(1, 3, 1),
                heat_with_lanes(2, 3, 1),
                heat_with_lanes(3, 3, 1),
            ],
        )],
    };
    let events = build_print_events(&meet, &HashSet::new(), &[], &no_abbreviations(), 1, false);

    let pages = build_timer_pages(&events, 1);

    let packed = pack_timer_pages(&pages[0].events, Some(2));
    assert_eq!(packed.len(), 2);
    assert!(matches!(packed[0][0], TimerLine::EventName(..)));
    assert!(matches!(packed[0][1], TimerLine::Divider));
    // The continuation page repeats the header before its remaining row.
    assert!(matches!(packed[1][0], TimerLine::EventName(..)));
    assert!(matches!(packed[1][1], TimerLine::Divider));
    assert!(matches!(packed[1][2], TimerLine::Row(..)));
}

#[test]
fn write_timer_pdf_produces_a_valid_pdf_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("meetmerger_timer_export_test.pdf");
    let pages = vec![
        TimerPage {
            lane: 1,
            events: vec![TimerEvent {
                event_name: "#1 Boys 10-11 25m Freestyle".to_string(),
                rows: vec![TimerRow {
                    heat_label: "Heat 1 of 1".to_string(),
                    swimmer: Some(TimerSwimmer {
                        last_name: "Doe".to_string(),
                        first_name: "Jane".to_string(),
                        age: 10,
                        team: "TST".to_string(),
                    }),
                }],
            }],
        },
        TimerPage {
            lane: 2,
            events: vec![],
        },
    ];
    write_timer_pdf("Test Meet", &pages, None, &path).expect("write_timer_pdf should succeed");

    let bytes = std::fs::read(&path).expect("file should exist");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 100);
    std::fs::remove_file(&path).ok();
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
                seed_time: SeedTime::Seconds(20.0),
            }],
        }],
        records: Vec::new(),
        number: 1,
    };
    write_pdf("Test Meet", &[print_event], false, None, &path).expect("write_pdf should succeed");

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
    let events = build_print_events(&meet, &HashSet::new(), &[], &abbreviations, 1, true);
    assert_eq!(events.len(), meet.events.len());

    let out_path = std::env::temp_dir().join("meetmerger_sample_export.pdf");
    write_pdf(&meet.title, &events, true, None, &out_path).expect("write_pdf should succeed");
    let bytes = std::fs::read(&out_path).expect("file should exist");
    assert!(bytes.starts_with(b"%PDF-"));
    println!(
        "wrote {} bytes, {} events, to {}",
        bytes.len(),
        events.len(),
        out_path.display()
    );

    let max_event = meet.events.iter().map(|e| e.number).max().unwrap_or(1);
    let rotated_events =
        build_print_events(&meet, &HashSet::new(), &[], &abbreviations, max_event, true);
    assert_eq!(
        rotated_events[0].event_name,
        events.last().unwrap().event_name
    );
}
