mod dialog;
mod state;
mod update;
mod view;

use std::path::PathBuf;

use clap::Parser;
use iced::Task;

use state::{Message, Wizard};
use update::update;
use view::view;

static INTER: &[u8] = include_bytes!("../../../fonts/Inter-Regular.ttf");

/// MeetMerger GUI wizard.
///
/// Ingests a Swimtopia Meet Maestro heat sheet PDF, or a CSV of lane entries,
/// and lets you recombine heats to maximize pool lane usage.
#[derive(Parser)]
struct Cli {
    /// Path to the heat sheet to load immediately, skipping the file picker.
    /// A `.csv` extension is loaded as a CSV of lane entries; anything else
    /// is loaded as a heat sheet PDF.
    path: Option<PathBuf>,

    /// Path to a corrections file, overriding the default `<path>.corrections.txt`.
    /// Only applies when loading a PDF.
    #[arg(long)]
    corrections: Option<PathBuf>,
}

fn boot(cli: &Cli) -> (Wizard, Task<Message>) {
    let mut wizard = Wizard::default();
    let Some(path) = &cli.path else {
        return (wizard, Task::none());
    };
    let is_csv = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if is_csv {
        wizard.csv_path = Some(path.clone());
    } else {
        wizard.pdf_path = Some(path.clone());
        wizard.corrections_path = cli
            .corrections
            .clone()
            .or_else(|| dialog::default_corrections_path(path));
    }
    (wizard, Task::done(Message::LoadMeet))
}

fn theme(_state: &Wizard) -> iced::Theme {
    iced::Theme::CatppuccinLatte
}

fn main() -> iced::Result {
    let cli = Cli::parse();
    iced::application(move || boot(&cli), update, view)
        .title("MeetMerger")
        .theme(theme)
        .font(INTER)
        .default_font(iced::Font::with_name("Inter"))
        .run()
}
