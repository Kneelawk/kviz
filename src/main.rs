#[macro_use]
extern crate tracing;

use crate::ffmpeg::encode::{EncoderFrame, EncoderState};
use crate::ffmpeg::AudioFormat;
use anyhow::Context;
use clap::Parser;
use ffmpeg_next::{format, frame};
use std::path::PathBuf;

mod ffmpeg;
mod recycle;

const AUDIO_FRAMES_IN_FLIGHT: usize = 8;
const VIDEO_FRAMES_IN_FLIGHT: usize = 8;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    ffmpeg::init_ffmpeg()?;

    let args = Args::parse();

    info!("Inputting from: {:?}", &args.input);
    info!("Outputting to: {:?}", &args.output);

    let blue = [0xFFu8, 0x0, 0x0, 0xFF].repeat(1920 * 1080);

    let (audio_producer, mut audio_consumer) = recycle::simple::recycler(
        (0..AUDIO_FRAMES_IN_FLIGHT)
            .map(|_| frame::Audio::empty())
            .collect(),
    )
    .await;

    let decoder_handle =
        ffmpeg::decode::DecoderHandle::spawn(args.input, AudioFormat::default(), audio_producer)
            .await
            .context("Spawning decoder handle")?;

    let encoder_handle = {
        let encoder_state = EncoderState::new(args.output, AudioFormat::default())
            .await
            .context("Creating encoder")?;

        let audio_format = AudioFormat::default();

        let (mut video_producer, video_consumer) = recycle::r#enum::enum_recycler(
            (0..AUDIO_FRAMES_IN_FLIGHT)
                .map(|_| {
                    EncoderFrame::Audio(frame::Audio::new(
                        audio_format.sample_format,
                        audio_format.frame_size.unwrap().get() as usize,
                        audio_format.channel_layout,
                    ))
                })
                .chain((0..VIDEO_FRAMES_IN_FLIGHT).map(|_| {
                    EncoderFrame::Video(frame::Video::new(format::Pixel::ARGB, 1920, 1080))
                }))
                .collect(),
        )
        .await;

        let handle = encoder_state.spawn(video_consumer);

        while let Some(mut audio) = audio_consumer.recv_data().await {
            {
                recv_recycling!(video_producer, video_holder, EncoderFrame::Audio(audio_out));

                audio.clone_into(audio_out);

                video_holder
                    .send()
                    .await
                    .context("Sending frame to encoder")?;
            }
            {
                recv_recycling!(video_producer, video_holder, EncoderFrame::Video(video_out));

                video_out.set_pts(audio.pts().map(|pts| pts * 24 / 48000));
                video_out.data_mut(0).copy_from_slice(&blue);

                video_holder.send().await.ok();
            }
            audio.send().await.ok();
        }

        info!("Closing...");

        handle
    };

    encoder_handle.join().await.context("Encoder handle")?;
    decoder_handle.join().await.context("Decoder handle")?;

    info!("Done.");

    Ok(())
}
