use super::*;

#[test]
fn parse_meet_attaches_a_record_shown_before_the_first_heat() {
    let text = "\
#11 Boys 8 & Under 25m Backstroke
FO Anthony Grimm
Fair Oaks Sharks
2011 18.16

Heat 1 of 1
1 LaPier, Liam 8 CP Cruisers 27.87
";
    let (meet, issues) = parse_meet(text);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    assert_eq!(meet.events.len(), 1);
    assert_eq!(meet.events[0].records.len(), 1);
    let record = &meet.events[0].records[0];
    assert_eq!(record.team_acronym, "FO");
    assert_eq!(record.swimmer_name, "Anthony Grimm");
    assert_eq!(record.team_name, "Fair Oaks Sharks");
    assert_eq!(record.year, 2011);
    assert_eq!(record.time, "18.16");
}

#[test]
fn parse_meet_leaves_records_empty_when_the_event_has_no_record_block() {
    let text = "\
#1 Boys 8 & Under 25m Freestyle
Heat 1 of 1
1 LaPier, Liam 8 CP Cruisers 27.87
";
    let (meet, issues) = parse_meet(text);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    assert!(meet.events[0].records.is_empty());
}

#[test]
fn parse_meet_reports_a_malformed_record_block_as_issues_instead_of_dropping_it() {
    let text = "\
#1 Boys 8 & Under 25m Freestyle
Some Garbled Line
Heat 1 of 1
1 LaPier, Liam 8 CP Cruisers 27.87
";
    let (meet, issues) = parse_meet(text);
    assert!(meet.events[0].records.is_empty());
    assert_eq!(issues.len(), 1);
}

#[test]
fn parse_meet_attaches_one_record_per_team_in_any_of_their_shapes() {
    // Real divisional heat sheets mix all of these shapes within one
    // event's record block: a combined one-liner, the classic
    // name/team-name/year-time trio, and a name followed by year and
    // time each on their own line with no team name shown at all.
    let text = "\
#11 Boys 8 & Under 25m Backstroke
NVSL Roman Lowery
2007
18.15

FO Anthony Grimm
Fair Oaks Sharks
2011 18.16

WC Nathaniel Temeles 2015 19.67

Heat 1 of 1
1 LaPier, Liam 8 CP Cruisers 27.87
";
    let (meet, issues) = parse_meet(text);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let records = &meet.events[0].records;
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].team_acronym, "NVSL");
    assert_eq!(records[0].swimmer_name, "Roman Lowery");
    assert_eq!(records[0].team_name, "");
    assert_eq!(records[0].year, 2007);
    assert_eq!(records[0].time, "18.15");

    assert_eq!(records[1].team_acronym, "FO");
    assert_eq!(records[1].team_name, "Fair Oaks Sharks");
    assert_eq!(records[1].year, 2011);

    assert_eq!(records[2].team_acronym, "WC");
    assert_eq!(records[2].swimmer_name, "Nathaniel Temeles");
    assert_eq!(records[2].team_name, "");
    assert_eq!(records[2].year, 2015);
    assert_eq!(records[2].time, "19.67");
}

#[test]
fn parse_meet_recovers_a_records_team_name_even_after_a_stray_blank_line() {
    // Some divisional heat sheets insert a spurious blank line between a
    // record's name line and the rest of its block — the scan is
    // sequential and content-based rather than position-based, so it
    // isn't thrown off by that extra blank.
    let text = "\
#11 Boys 8 & Under 25m Backstroke
L Henry Rossman

Langley Wildthings
2026 31.69

Heat 1 of 1
1 LaPier, Liam 8 CP Cruisers 27.87
";
    let (meet, issues) = parse_meet(text);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let records = &meet.events[0].records;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].team_acronym, "L");
    assert_eq!(records[0].swimmer_name, "Henry Rossman");
    assert_eq!(records[0].team_name, "Langley Wildthings");
    assert_eq!(records[0].year, 2026);
    assert_eq!(records[0].time, "31.69");
}

#[test]
fn parse_meet_ignores_alternates_between_an_events_heats_and_the_next_event() {
    let text = "\
#1 Boys 8 & Under 25m Freestyle
Heat 1 of 1
1 LaPier, Liam 8 CP Cruisers 27.87

Alternates
Bamberger, Weston 7 Langley 26.37
Gaughan, Lincoln 6 WC Wahoos 26.59

#2 Girls 8 & Under 25m Freestyle
Heat 1 of 1
1 Doe, Jane 7 Sharks 32.10
";
    let (meet, issues) = parse_meet(text);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    assert_eq!(meet.events.len(), 2);
    assert_eq!(meet.events[0].heats[0].lanes.len(), 1);
    assert_eq!(meet.events[1].number, 2);
    assert_eq!(meet.events[1].heats[0].lanes[0].number, 1);
}

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

#[test]
fn parse_meet_csv_attaches_a_record_from_its_five_optional_columns() {
    let data = "\
event name,heat,lane,name,age,team,entry time,record team,record name,record year,record time,record team name
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,1,\"LaPier, Liam\",8,CP Cruisers,27.87,FO,Anthony Grimm,2011,18.16,Fair Oaks Sharks
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,2,\"Doe, Jane\",8,Sharks,28.00,,,,,
";
    let (meet, issues) = parse_meet_csv(data, "My Meet");
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    assert_eq!(meet.events[0].records.len(), 1);
    let record = &meet.events[0].records[0];
    assert_eq!(record.team_acronym, "FO");
    assert_eq!(record.swimmer_name, "Anthony Grimm");
    assert_eq!(record.year, 2011);
    assert_eq!(record.time, "18.16");
    assert_eq!(record.team_name, "Fair Oaks Sharks");
}

#[test]
fn parse_meet_csv_leaves_records_empty_without_the_optional_columns() {
    let data = "\
event name,heat,lane,name,age,team,entry time
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,1,\"LaPier, Liam\",8,CP Cruisers,27.87
";
    let (meet, issues) = parse_meet_csv(data, "My Meet");
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    assert!(meet.events[0].records.is_empty());
}

#[test]
fn parse_meet_csv_attaches_one_record_per_team_from_multiple_rows() {
    let data = "\
event name,heat,lane,name,age,team,entry time,record team,record name,record year,record time,record team name
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,1,\"LaPier, Liam\",8,CP Cruisers,27.87,FO,Anthony Grimm,2011,18.16,Fair Oaks Sharks
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,2,\"Doe, Jane\",8,Sharks,28.00,WC,Nathaniel Temeles,2015,19.67,WC Wahoos
";
    let (meet, issues) = parse_meet_csv(data, "My Meet");
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let records = &meet.events[0].records;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].team_acronym, "FO");
    assert_eq!(records[1].team_acronym, "WC");
}
