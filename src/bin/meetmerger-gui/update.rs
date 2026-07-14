use iced::Task;

use meetmerger::merge::{self, MixedHeatSource};
use meetmerger::model::Heat;

use crate::dialog;
use crate::state::{Message, Step, Wizard};

pub fn update(state: &mut Wizard, message: Message) -> Task<Message> {
    match message {
        Message::PickPdf => Task::perform(dialog::pick_pdf(), Message::PdfPicked),
        Message::PdfPicked(path) => {
            if let Some(path) = path {
                state.corrections_path = dialog::default_corrections_path(&path);
                state.pdf_path = Some(path);
            }
            Task::none()
        }
        Message::PickCorrections => {
            Task::perform(dialog::pick_corrections(), Message::CorrectionsPicked)
        }
        Message::CorrectionsPicked(path) => {
            if path.is_some() {
                state.corrections_path = path;
            }
            Task::none()
        }
        Message::LoadMeet => {
            let Some(pdf_path) = state.pdf_path.clone() else {
                return Task::none();
            };
            state.is_loading = true;
            state.load_error = None;
            let corrections_path = state.corrections_path.clone();
            Task::perform(
                dialog::load_and_parse(pdf_path, corrections_path),
                Message::MeetLoaded,
            )
        }
        Message::MeetLoaded(result) => {
            state.is_loading = false;
            match result {
                Ok((meet, issues)) => {
                    state.lane_capacity = merge::infer_lane_capacity(&meet);
                    state.meet = Some(meet);
                    state.issues = issues;
                    state.step = Step::Review;
                }
                Err(err) => state.load_error = Some(err),
            }
            Task::none()
        }
        Message::GoToReview => {
            state.step = Step::Review;
            Task::none()
        }
        Message::GoToSelectHeats => {
            state.step = Step::SelectHeats;
            Task::none()
        }
        Message::ToggleHeatSelected(event_number, heat_number) => {
            let key = (event_number, heat_number);
            if !state.selection.remove(&key) {
                state.selection.insert(key);
            }
            Task::none()
        }
        Message::ConfirmSelection => confirm_selection(state),
        Message::HeaderEdited(header) => {
            if let Some(pending) = &mut state.pending {
                pending.header = header;
            }
            Task::none()
        }
        Message::ConfirmMixedHeat => {
            if let Some(pending) = state.pending.take() {
                for source in &pending.sources {
                    state
                        .consumed
                        .insert((source.event_number, source.heat_number));
                }
                state.mixed_heats.push(pending);
            }
            state.selection.clear();
            state.step = Step::SelectHeats;
            Task::none()
        }
        Message::CancelMixedHeat => {
            state.pending = None;
            state.step = Step::SelectHeats;
            Task::none()
        }
        Message::Finish => {
            state.step = Step::FinalPreview;
            Task::none()
        }
        Message::BackToSelectHeats => {
            state.step = Step::SelectHeats;
            Task::none()
        }
        Message::RenameMixedHeat(index, header) => {
            if let Some(mixed) = state.mixed_heats.get_mut(index) {
                mixed.header = header;
            }
            Task::none()
        }
    }
}

fn confirm_selection(state: &mut Wizard) -> Task<Message> {
    let Some(meet) = &state.meet else {
        return Task::none();
    };

    let mut sources: Vec<(MixedHeatSource, &Heat)> = Vec::new();
    for event in &meet.events {
        for heat in &event.heats {
            let key = (event.number, heat.number);
            if state.selection.contains(&key) {
                sources.push((
                    MixedHeatSource {
                        event_number: event.number,
                        heat_number: heat.number,
                        gender: event.gender.clone(),
                        distance_m: event.distance_m,
                        stroke: event.stroke.clone(),
                    },
                    heat,
                ));
            }
        }
    }

    let heats: Vec<&Heat> = sources.iter().map(|(_, heat)| *heat).collect();
    if !merge::can_merge(&heats, state.lane_capacity) {
        return Task::none();
    }

    state.pending = Some(merge::build_mixed_heat(sources, state.lane_capacity));
    state.step = Step::MixedHeatEdit;
    Task::none()
}
