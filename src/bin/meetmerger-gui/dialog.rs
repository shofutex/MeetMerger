use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use meetmerger::export;
use meetmerger::merge::MixedHeat;
use meetmerger::model::Meet;
use meetmerger::parse::{self, Issue};

pub async fn pick_pdf() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("Heat sheet PDF", &["pdf"])
        .pick_file()
        .await
        .map(|f| f.path().to_path_buf())
}

pub async fn pick_corrections() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("Corrections", &["txt"])
        .pick_file()
        .await
        .map(|f| f.path().to_path_buf())
}

// Mirrors the CLI's default: <pdf>.corrections.txt next to the heat sheet, if it exists.
pub fn default_corrections_path(pdf_path: &Path) -> Option<PathBuf> {
    let candidate = pdf_path.with_extension("corrections.txt");
    candidate.exists().then_some(candidate)
}

fn load_corrections(path: &Path) -> Result<Vec<(String, String)>, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(find, replace)| (find.to_string(), replace.to_string()))
        .collect())
}

pub async fn load_and_parse(
    pdf_path: PathBuf,
    corrections_path: Option<PathBuf>,
) -> Result<(Meet, Vec<Issue>), String> {
    let raw = pdf_extract::extract_text(&pdf_path).map_err(|e| e.to_string())?;
    let corrections = match &corrections_path {
        Some(path) if path.exists() => load_corrections(path)?,
        _ => Vec::new(),
    };
    let text = parse::apply_corrections(&parse::normalize_corruption(&raw), &corrections);
    Ok(parse::parse_meet(&text))
}

pub async fn pick_save_path(default_name: String) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_file_name(default_name)
        .add_filter("PDF", &["pdf"])
        .save_file()
        .await
        .map(|f| f.path().to_path_buf())
}

pub async fn export_pdf(
    meet: Meet,
    consumed: HashSet<(u32, u32)>,
    mixed_heats: Vec<MixedHeat>,
    abbreviations: HashMap<String, String>,
    start_event: u32,
    path: PathBuf,
) -> Result<PathBuf, String> {
    let events =
        export::build_print_events(&meet, &consumed, &mixed_heats, &abbreviations, start_event);
    export::write_pdf(&meet.title, &events, &path)?;
    Ok(path)
}

#[allow(clippy::too_many_arguments)]
pub async fn export_timer_sheets(
    meet: Meet,
    consumed: HashSet<(u32, u32)>,
    mixed_heats: Vec<MixedHeat>,
    abbreviations: HashMap<String, String>,
    start_event: u32,
    lane_capacity: u32,
    heats_per_page: Option<u32>,
    path: PathBuf,
) -> Result<PathBuf, String> {
    let events =
        export::build_print_events(&meet, &consumed, &mixed_heats, &abbreviations, start_event);
    let pages = export::build_timer_pages(&events, lane_capacity);
    export::write_timer_pdf(&meet.title, &pages, heats_per_page, &path)?;
    Ok(path)
}
