#[macro_use]
extern crate tracing;

use anyhow::Context;
use clap::Parser;
use ffmpeg_next::format;
use std::path::PathBuf;

mod ffmpeg;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    ffmpeg::init_ffmpeg()?;

    let args = Args::parse();

    info!("Inputting from {:?}", &args.input);

    let input = format::input(&args.input).context("Opening input file")?;
    format::context::input::dump(&input, 0, Some(&args.input.to_string_lossy()));

    Ok(())
}
