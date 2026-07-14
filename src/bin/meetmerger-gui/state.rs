use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use meetmerger::merge::MixedHeat;
use meetmerger::model::Meet;
use meetmerger::parse::Issue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Step {
    #[default]
    Load,
    Review,
    SelectHeats,
    MixedHeatEdit,
    FinalPreview,
    TeamAbbreviations,
}

#[derive(Default)]
pub struct Wizard {
    pub step: Step,
    pub pdf_path: Option<PathBuf>,
    pub corrections_path: Option<PathBuf>,
    pub is_loading: bool,
    pub load_error: Option<String>,
    pub meet: Option<Meet>,
    pub issues: Vec<Issue>,
    pub lane_capacity: u32,
    // (event_number, heat_number) already folded into a mixed heat
    pub consumed: HashSet<(u32, u32)>,
    pub mixed_heats: Vec<MixedHeat>,
    // in-progress picks for the next mixed heat
    pub selection: HashSet<(u32, u32)>,
    pub pending: Option<MixedHeat>,

    pub export_start_event: String,
    pub team_abbreviations: HashMap<String, String>,
    pub is_exporting: bool,
    pub export_result: Option<Result<PathBuf, String>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PickPdf,
    PdfPicked(Option<PathBuf>),
    PickCorrections,
    CorrectionsPicked(Option<PathBuf>),
    LoadMeet,
    MeetLoaded(Result<(Meet, Vec<Issue>), String>),

    GoToReview,
    GoToSelectHeats,

    ToggleHeatSelected(u32, u32),
    ConfirmSelection,
    HeaderEdited(String),
    ConfirmMixedHeat,
    CancelMixedHeat,

    Finish,
    BackToSelectHeats,
    RenameMixedHeat(usize, String),

    StartEventChanged(String),
    GoToTeamAbbreviations,
    BackToFinalPreview,
    TeamAbbreviationChanged(String, String),
    ExportPdf,
    ExportPathPicked(Option<PathBuf>),
    PdfExported(Result<PathBuf, String>),
}
