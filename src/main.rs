// Copyright © 2025 William Gacquer — tous droits réservés

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "pmsi2json", about = "Convert PMSI .grp files to JSON", version)]
struct Args {
    /// Input .grp file or directory containing .grp files
    input: PathBuf,

    /// Write JSON to a file instead of stdout
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Pretty-print JSON output
    #[arg(long, conflicts_with = "lines")]
    pretty: bool,

    /// One compact JSON object per input line (NDJSON), incompatible with --pretty
    #[arg(long)]
    lines: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let result = pmsi2json::convert_path(&args.input)?;
    if args.lines {
        pmsi2json::write_json_lines(&result, args.output.as_deref())
    } else {
        pmsi2json::write_json(&result, args.output.as_deref(), args.pretty)
    }
}
