use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StressKind {
    Switches,
    Routes,
}

#[derive(Clone, Debug, Parser)]
#[command(
    name = "tokenmaster-m0",
    version,
    about = "TokenMaster M0 native probe"
)]
pub struct Args {
    #[arg(long, value_enum)]
    pub stress: Option<StressKind>,
    #[arg(long, default_value_t = 10_000, value_parser = parse_iterations)]
    pub iterations: u32,
    #[arg(long, default_value_t = 0, value_parser = parse_rows)]
    pub rows: u64,
    #[arg(long, default_value_t = 2, value_parser = parse_duration)]
    pub duration_seconds: u64,
    #[arg(long)]
    pub report: Option<PathBuf>,
}

fn parse_duration(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "duration must be an integer".to_owned())?;
    if (1..=259_200).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("duration must be within 1..=259200 seconds".to_owned())
    }
}

fn parse_iterations(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "iterations must be an integer".to_owned())?;
    if (1..=100_000).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("iterations must be within 1..=100000".to_owned())
    }
}

fn parse_rows(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "rows must be an integer".to_owned())?;
    if parsed <= 1_000_000 {
        Ok(parsed)
    } else {
        Err("rows must be within 0..=1000000".to_owned())
    }
}
