#[macro_use]
extern crate tracing;

use crate::ffmpeg::AudioFormat;
use anyhow::Context;
use clap::Parser;
use ffmpeg_next::frame;
use std::ops::Deref;
use std::path::PathBuf;

mod ffmpeg;
mod recycler;

const AUDIO_FRAMES_IN_FLIGHT: usize = 8;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    ffmpeg::init_ffmpeg()?;

    let args = Args::parse();

    info!("Inputting from {:?}", &args.input);

    let (producer, mut consumer) = recycler::recycler(
        (0..AUDIO_FRAMES_IN_FLIGHT)
            .map(|_| frame::Audio::empty())
            .collect(),
    )
    .await;

    let handle = ffmpeg::decode::DecoderHandle::spawn(args.input, AudioFormat::default(), producer)
        .await
        .context("Spawning decoder handle")?;

    while let Some(mut audio) = consumer.recv_data().await {
        info!("Frame: {:?}", audio.deref());

        audio.send().await.context("Sending recycling")?;
    }

    info!("Closing...");

    handle.join().await.context("Decoder handle")?;

    info!("Done.");

    Ok(())
}
