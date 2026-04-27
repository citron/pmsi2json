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
    #[arg(long)]
    pretty: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let result = pmsi2json::convert_path(&args.input)?;
    pmsi2json::write_json(&result, args.output.as_deref(), args.pretty)
}
