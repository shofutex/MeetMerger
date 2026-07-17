use iced::widget::{button, checkbox, column, container, row, rule, scrollable, text, text_input};
use iced::{Color, Element, Length};

use meetmerger::export;
use meetmerger::merge::{self, MixedHeat};
use meetmerger::model::{Event, Heat, Lane};

use crate::state::{Message, Step, Wizard};

pub fn view(state: &Wizard) -> Element<'_, Message> {
    let (body, actions) = match state.step {
        Step::Load => view_load(state),
        Step::Review => view_review(state),
        Step::SelectHeats => view_select_heats(state),
        Step::MixedHeatEdit => view_mixed_heat_edit(state),
        Step::FinalPreview => view_final_preview(state),
        Step::TeamAbbreviations => view_team_abbreviations(state),
    };

    container(
        column![
            scrollable(body).width(Length::Fill).height(Length::Fill),
            actions,
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(16)
    .into()
}

const COLOR_ERROR: Color = Color::from_rgb(0.75, 0.15, 0.15);
const COLOR_SUCCESS: Color = Color::from_rgb(0.16, 0.5, 0.2);
const COLOR_WARNING: Color = Color::from_rgb(0.7, 0.45, 0.0);

fn lane_line(lane: &Lane) -> String {
    match &lane.swimmer {
        Some(s) => {
            let exh = if s.exhibition { " EXH" } else { "" };
            format!(
                "Lane {}: {}, {} ({}){} - {} - {}",
                lane.number, s.last_name, s.first_name, s.age, exh, s.team, s.seed_time
            )
        }
        None => format!("Lane {}: (empty)", lane.number),
    }
}

fn heat_header(heat: &Heat) -> String {
    format!("Heat {} of {}", heat.number, heat.of)
}

fn event_header(event: &Event) -> String {
    format!(
        "#{} {} {} {}m {}",
        event.number, event.gender, event.age_group, event.distance_m, event.stroke
    )
}

fn view_load(state: &Wizard) -> (Element<'_, Message>, Element<'_, Message>) {
    let pdf_label = match &state.pdf_path {
        Some(path) => format!("PDF: {}", path.display()),
        None => "No PDF selected".to_string(),
    };
    let corrections_label = match &state.corrections_path {
        Some(path) => format!("Corrections: {}", path.display()),
        None => "No corrections file".to_string(),
    };

    let mut col = column![
        text("MeetMerger").size(28),
        row![
            button("Choose heat sheet PDF...").on_press(Message::PickPdf),
            text(pdf_label),
        ]
        .spacing(12),
        row![
            button("Choose corrections file...").on_press(Message::PickCorrections),
            text(corrections_label),
        ]
        .spacing(12),
    ]
    .spacing(12);

    if state.is_loading {
        col = col.push(text("Loading..."));
    }

    if let Some(err) = &state.load_error {
        col = col.push(text(format!("Error: {err}")).color(COLOR_ERROR));
    }

    if !state.issues.is_empty() {
        col =
            col.push(text(format!("{} issue(s) found:", state.issues.len())).color(COLOR_WARNING));
        for issue in &state.issues {
            col = col.push(text(issue.to_string()).color(COLOR_WARNING));
        }
    }

    let load_button = if state.pdf_path.is_some() && !state.is_loading {
        button("Load").on_press(Message::LoadMeet)
    } else {
        button("Load")
    };
    let mut actions = row![load_button].spacing(12);
    if state.meet.is_some() {
        actions = actions.push(button("Continue").on_press(Message::GoToReview));
    }

    (col.into(), actions.into())
}

fn view_review(state: &Wizard) -> (Element<'_, Message>, Element<'_, Message>) {
    let Some(meet) = &state.meet else {
        return (
            text("Nothing loaded yet.").into(),
            row![].spacing(12).into(),
        );
    };

    let mut col = column![text(format!("{} — {}", meet.title, meet.date)).size(24)].spacing(14);

    for event in &meet.events {
        let mut event_col = column![text(event_header(event)).size(18)].spacing(4);
        for heat in &event.heats {
            event_col = event_col.push(text(format!("  {}", heat_header(heat))));
            for lane in &heat.lanes {
                event_col = event_col.push(text(format!("    {}", lane_line(lane))));
            }
        }
        col = col.push(event_col);
        col = col.push(rule::horizontal(1));
    }

    let actions = row![button("Continue to merge").on_press(Message::GoToSelectHeats)].spacing(12);
    (col.into(), actions.into())
}

fn view_select_heats(state: &Wizard) -> (Element<'_, Message>, Element<'_, Message>) {
    let Some(meet) = &state.meet else {
        return (
            text("Nothing loaded yet.").into(),
            row![].spacing(12).into(),
        );
    };

    let mut col = column![text(format!("Pool lane capacity: {}", state.lane_capacity))].spacing(12);
    let mut selected_count = 0usize;
    let mut selected_heats: Vec<&Heat> = Vec::new();

    for event in &meet.events {
        let mut event_col = column![text(event_header(event)).size(16)].spacing(4);
        for heat in &event.heats {
            let key = (event.number, heat.number);
            let swimmer_count = merge::heat_swimmer_count(heat);
            if state.selection.contains(&key) {
                selected_count += swimmer_count;
                selected_heats.push(heat);
            }

            let label = format!(
                "{} ({}/{})",
                heat_header(heat),
                swimmer_count,
                state.lane_capacity
            );

            let row_el: Element<'_, Message> = if state.consumed.contains(&key) {
                text(format!("{label} — used in a mixed heat")).into()
            } else if merge::is_heat_eligible(heat, state.lane_capacity) {
                checkbox(state.selection.contains(&key))
                    .label(label)
                    .on_toggle(move |_| Message::ToggleHeatSelected(key.0, key.1))
                    .into()
            } else {
                text(format!("{label} — full")).into()
            };
            event_col = event_col.push(row_el);
        }
        col = col.push(event_col);
        col = col.push(rule::horizontal(1));
    }

    let heats_needed = selected_count.div_ceil(state.lane_capacity.max(1) as usize);
    let totals_label = if heats_needed > 1 {
        format!(
            "Selected: {selected_count} swimmers — will split into {heats_needed} mixed heats of up to {}",
            state.lane_capacity
        )
    } else {
        format!("Selected: {selected_count}/{}", state.lane_capacity)
    };

    let can_merge = merge::can_merge(&selected_heats);
    // Shown outside the scrollable event list, so the running total stays
    // visible while checking boxes further down a long list.
    let mut actions = row![].spacing(12);
    actions = actions.push(if can_merge {
        button("Create mixed heat(s)").on_press(Message::ConfirmSelection)
    } else {
        button("Create mixed heat(s)")
    });
    actions = actions.push(if !state.mixed_heats.is_empty() {
        button("Finish").on_press(Message::Finish)
    } else {
        button("Finish")
    });
    actions = actions.push(text(totals_label));

    (col.into(), actions.into())
}

fn view_mixed_heat_edit(state: &Wizard) -> (Element<'_, Message>, Element<'_, Message>) {
    if state.pending.is_empty() {
        return (
            text("No mixed heat in progress.").into(),
            row![].spacing(12).into(),
        );
    }

    let mut col = column![text("Mixed heat header(s)").size(18)].spacing(14);

    for (index, pending) in state.pending.iter().enumerate() {
        let mut heat_col = column![
            text_input("Mixed heat header", &pending.header)
                .on_input(move |header| Message::HeaderEdited(index, header)),
        ]
        .spacing(2);
        for lane in &pending.lanes {
            heat_col = heat_col.push(text(lane_line(lane)));
        }
        col = col.push(heat_col);
        col = col.push(rule::horizontal(1));
    }

    let actions = row![
        button("Confirm").on_press(Message::ConfirmMixedHeat),
        button("Cancel").on_press(Message::CancelMixedHeat),
    ]
    .spacing(12);

    (col.into(), actions.into())
}

fn view_final_preview(state: &Wizard) -> (Element<'_, Message>, Element<'_, Message>) {
    let Some(meet) = &state.meet else {
        return (
            text("Nothing loaded yet.").into(),
            row![].spacing(12).into(),
        );
    };

    let mut col = column![text("Final preview").size(24)].spacing(14);

    for event in &meet.events {
        let remaining: Vec<&Heat> = event
            .heats
            .iter()
            .filter(|h| !state.consumed.contains(&(event.number, h.number)))
            .collect();

        let mut event_col = column![].spacing(4);
        let mut has_content = false;

        if !remaining.is_empty() {
            has_content = true;
            event_col = event_col.push(text(event_header(event)).size(16));
            for heat in remaining {
                event_col = event_col.push(text(format!("  {}", heat_header(heat))));
                for lane in &heat.lanes {
                    event_col = event_col.push(text(format!("    {}", lane_line(lane))));
                }
            }
        }

        // Mixed heats appear right after the earliest event they draw from.
        for (index, mixed) in state.mixed_heats.iter().enumerate() {
            if mixed.anchor_event() == event.number {
                has_content = true;
                event_col = event_col.push(mixed_heat_view(index, mixed));
            }
        }

        if has_content {
            col = col.push(event_col);
            col = col.push(rule::horizontal(1));
        }
    }

    let actions = row![
        button("Back").on_press(Message::BackToSelectHeats),
        button("Export PDFs").on_press(Message::GoToTeamAbbreviations),
    ]
    .spacing(12);
    (col.into(), actions.into())
}

fn view_team_abbreviations(state: &Wizard) -> (Element<'_, Message>, Element<'_, Message>) {
    let Some(meet) = &state.meet else {
        return (
            text("Nothing loaded yet.").into(),
            row![].spacing(12).into(),
        );
    };

    let mut col =
        column![
            text("Optional team abbreviations for the printed PDF (blank = full name):").size(18)
        ]
        .spacing(8);

    for team in export::distinct_teams(meet, &state.consumed, &state.mixed_heats) {
        let value = state
            .team_abbreviations
            .get(&team)
            .cloned()
            .unwrap_or_default();
        col = col.push(
            row![
                text(team.clone()),
                text_input("abbreviation", &value)
                    .on_input(move |v| Message::TeamAbbreviationChanged(team.clone(), v)),
            ]
            .spacing(12),
        );
    }

    col = col.push(
        row![
            text("Start event # (IM Carnival order, optional):"),
            text_input("1", &state.export_start_event).on_input(Message::StartEventChanged),
        ]
        .spacing(12),
    );

    col = col.push(
        row![
            text("Heats per page for timer sheets (optional):"),
            text_input("unlimited", &state.heats_per_page).on_input(Message::HeatsPerPageChanged),
        ]
        .spacing(12),
    );

    if state.is_exporting {
        col = col.push(text("Exporting heat sheet..."));
    }
    if let Some(result) = &state.export_result {
        match result {
            Ok(path) => {
                col = col.push(
                    text(format!("Heat sheet saved to {}", path.display())).color(COLOR_SUCCESS),
                )
            }
            Err(err) => {
                col = col.push(text(format!("Heat sheet export failed: {err}")).color(COLOR_ERROR))
            }
        }
    }

    if state.is_exporting_timers {
        col = col.push(text("Exporting timer sheets..."));
    }
    if let Some(result) = &state.timer_export_result {
        match result {
            Ok(path) => {
                col = col.push(
                    text(format!("Timer sheets saved to {}", path.display())).color(COLOR_SUCCESS),
                )
            }
            Err(err) => {
                col =
                    col.push(text(format!("Timer sheets export failed: {err}")).color(COLOR_ERROR))
            }
        }
    }

    let export_button = if state.is_exporting {
        button("Export Heat Sheet")
    } else {
        button("Export Heat Sheet").on_press(Message::ExportPdf)
    };
    let timer_button = if state.is_exporting_timers {
        button("Export Timer Sheets")
    } else {
        button("Export Timer Sheets").on_press(Message::ExportTimerSheets)
    };
    let actions = row![
        button("Back").on_press(Message::BackToFinalPreview),
        export_button,
        timer_button,
    ]
    .spacing(12);
    (col.into(), actions.into())
}

fn mixed_heat_view(index: usize, mixed: &MixedHeat) -> Element<'_, Message> {
    let mut col = column![text_input("Mixed heat header", &mixed.header)
        .on_input(move |header| Message::RenameMixedHeat(index, header))]
    .spacing(2);
    for lane in &mixed.lanes {
        col = col.push(text(format!("  {}", lane_line(lane))));
    }
    col.into()
}
