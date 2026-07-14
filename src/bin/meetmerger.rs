use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use meetmerger::model::Meet;
use meetmerger::parse::{self, Issue};

/// Ingest a Swimtopia Meet Maestro heat sheet PDF and print the parsed
/// events/heats/lanes back out so the import can be validated against the
/// source PDF.
#[derive(Parser)]
struct Cli {
    /// Path to the heat sheet PDF
    path: PathBuf,

    /// Path to a corrections file (`garbled text=fixed text` per line) used
    /// to patch names the PDF's font couldn't render. Defaults to
    /// `<path>.corrections.txt` next to the heat sheet, if it exists.
    #[arg(long)]
    corrections: Option<PathBuf>,
}

fn load_corrections(path: &PathBuf) -> Result<Vec<(String, String)>> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read corrections file {}", path.display()))?;
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(find, replace)| (find.to_string(), replace.to_string()))
        .collect())
}

fn print_meet(meet: &Meet) {
    println!("{} — {}", meet.title, meet.date);
    for event in &meet.events {
        println!(
            "\n#{} {} {} {}m {}",
            event.number, event.gender, event.age_group, event.distance_m, event.stroke
        );
        for heat in &event.heats {
            println!("  Heat {} of {}", heat.number, heat.of);
            for lane in &heat.lanes {
                match &lane.swimmer {
                    Some(s) => {
                        let exh = if s.exhibition { " EXH" } else { "" };
                        println!(
                            "    Lane {}: {}, {} ({}){} - {} - {}",
                            lane.number, s.last_name, s.first_name, s.age, exh, s.team, s.seed_time
                        );
                    }
                    None => println!("    Lane {}: (empty)", lane.number),
                }
            }
        }
    }
}

fn print_issues(issues: &[Issue], corrections_path: &std::path::Path) {
    if issues.is_empty() {
        return;
    }
    println!("\n--- {} issue(s) found ---", issues.len());
    for issue in issues {
        println!("  {issue}");
    }
    println!(
        "\nTo patch unresolved characters, add \"garbled=fixed\" lines to {} and re-run.",
        corrections_path.display()
    );
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let raw_text = pdf_extract::extract_text(&cli.path)
        .with_context(|| format!("failed to extract text from {}", cli.path.display()))?;

    let corrections_path = cli
        .corrections
        .clone()
        .unwrap_or_else(|| cli.path.with_extension("corrections.txt"));
    let corrections = if corrections_path.exists() {
        load_corrections(&corrections_path)?
    } else {
        Vec::new()
    };

    let text = parse::normalize_corruption(&raw_text);
    let text = parse::apply_corrections(&text, &corrections);
    let (meet, issues) = parse::parse_meet(&text);

    print_meet(&meet);
    print_issues(&issues, &corrections_path);

    Ok(())
}
