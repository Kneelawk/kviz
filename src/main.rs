#[macro_use]
extern crate tracing;

use crate::ffmpeg::encode::{EncoderFrame, EncoderState};
use crate::ffmpeg::AudioFormat;
use anyhow::Context;
use clap::Parser;
use ffmpeg_next::{format, frame};
use realfft::RealFftPlanner;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

    let audio_format = AudioFormat::default();

    let mut fft_planner = RealFftPlanner::<f32>::new();
    let fft = fft_planner.plan_fft_forward(audio_format.frame_size.unwrap().get() as usize);
    let mut fft_input = fft.make_input_vec();
    let mut fft_output = fft.make_output_vec();
    let mut fft_scratch = fft.make_scratch_vec();

    let (audio_producer, mut audio_consumer) = recycle::simple::recycler(
        (0..AUDIO_FRAMES_IN_FLIGHT)
            .map(|_| frame::Audio::empty())
            .collect(),
    )
    .await;

    let decoder_handle =
        ffmpeg::decode::DecoderHandle::spawn(args.input, audio_format, audio_producer)
            .await
            .context("Spawning decoder handle")?;

    let encoder_handle = {
        let encoder_state = EncoderState::new(args.output, audio_format)
            .await
            .context("Creating encoder")?;

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

        let mut last_msg = Instant::now();
        while let Some(mut audio) = audio_consumer.recv_data().await {
            {
                recv_recycling!(video_producer, video_holder, EncoderFrame::Audio(audio_out));

                if audio_out.samples() > audio.samples() {
                    for i in 0..audio_out.planes() {
                        audio_out.plane_mut(i).fill(0f32);
                    }
                }

                audio.clone_into(audio_out);

                video_holder
                    .send()
                    .await
                    .context("Sending frame to encoder")?;
            }
            {
                let plane = audio.plane::<f32>(0);
                if plane.len() == fft_input.len() {
                    fft_input.copy_from_slice(plane);
                } else {
                    fft_input.fill(0.0);
                    fft_input[0..plane.len()].copy_from_slice(plane);
                }
                fft.process_with_scratch(&mut fft_input, &mut fft_output, &mut fft_scratch)
                    .context("Performing FFT")?;

                recv_recycling!(video_producer, video_holder, EncoderFrame::Video(video_out));

                let video_frame = video_out.data_mut(0);

                for i in 0usize..1920 {
                    let index = i * fft_output.len() / 1920;
                    let out = fft_output[index].re;

                    let color = (out * 255.0) as u8;

                    for y in 0usize..1080 {
                        let pixel = (y * 1920 + i) * 4;
                        video_frame[pixel] = 0xFF;
                        video_frame[pixel + 3] = color;
                    }
                }

                video_out.set_pts(audio.pts().map(|pts| pts * 24 / 48000));

                video_holder.send().await.ok();
            }

            let now = Instant::now();
            if now - last_msg > Duration::from_secs(2) {
                last_msg = now;
                info!("Secs: {}", audio.pts().unwrap() / 48000)
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
