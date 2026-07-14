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

/// MeetMerger GUI wizard.
#[derive(Parser)]
struct Cli {
    /// Path to the heat sheet PDF to load immediately, skipping the file picker.
    path: Option<PathBuf>,

    /// Path to a corrections file, overriding the default `<path>.corrections.txt`.
    #[arg(long)]
    corrections: Option<PathBuf>,
}

fn boot(cli: &Cli) -> (Wizard, Task<Message>) {
    let mut wizard = Wizard::default();
    let Some(path) = &cli.path else {
        return (wizard, Task::none());
    };
    wizard.pdf_path = Some(path.clone());
    wizard.corrections_path = cli
        .corrections
        .clone()
        .or_else(|| dialog::default_corrections_path(path));
    (wizard, Task::done(Message::LoadMeet))
}

fn main() -> iced::Result {
    let cli = Cli::parse();
    iced::application(move || boot(&cli), update, view)
        .title("MeetMerger")
        .run()
}
