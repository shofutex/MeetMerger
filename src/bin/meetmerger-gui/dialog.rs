use std::path::{Path, PathBuf};

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
