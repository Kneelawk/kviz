use crate::ffmpeg::decode::DecoderHandle;
use crate::ffmpeg::encode::{EncoderArgs, EncoderFrame, EncoderState};
use crate::ffmpeg::AudioFormat;
use crate::recv_recycling;
use crate::recycle::r#enum::enum_recycler;
use crate::recycle::simple::recycler;
use crate::util::MultiSlice;
use crate::visualizer::bars::BarsVisualizerInput;
use crate::visualizer::cotton::CottonVisualizerInput;
use crate::visualizer::credits::CreditsVisualizerInput;
use crate::visualizer::{Visualizer, VisualizerInput, VisualizerInputExtra};
use anyhow::{bail, Context};
use ffmpeg_next::{format, frame};
use realfft::RealFftPlanner;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;

const AUDIO_FRAMES_IN_FLIGHT: usize = 8;
const VIDEO_FRAMES_IN_FLIGHT: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub program: Program,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub width: u32,
    pub height: u32,
    pub visualizer: VisualizerEnum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VisualizerEnum {
    Bars(BarsVisualizerInput),
    Cotton(CottonVisualizerInput),
    Credits(CreditsVisualizerInput),
}

impl VisualizerEnum {
    async fn new_visualizer(
        &self,
        extra: VisualizerInputExtra,
    ) -> anyhow::Result<Box<dyn Visualizer>> {
        match self {
            VisualizerEnum::Bars(input) => input.new_visualizer(extra).await,
            VisualizerEnum::Cotton(input) => input.new_visualizer(extra).await,
            VisualizerEnum::Credits(input) => input.new_visualizer(extra).await,
        }
    }
}

impl Project {
    pub async fn visualize(&self) -> anyhow::Result<()> {
        let Some(input_file) = self.input.as_ref() else {
            bail!(VisualizeError::NoInputFile)
        };
        let Some(output_file) = self.output.as_ref() else {
            bail!(VisualizeError::NoOutputFile)
        };

        let program = &self.program;

        info!("Starting visualization...");

        info!("Inputting from: {:?}", input_file);
        info!("Outputting from: {:?}", output_file);

        let audio_format = AudioFormat::default();

        let mut fft_planner = RealFftPlanner::<f32>::new();
        let fft = fft_planner.plan_fft_forward(audio_format.frame_size.unwrap().get() as usize);
        let mut fft_input = fft.make_input_vec();
        let mut fft_output = MultiSlice::new(
            (0..audio_format.channel_layout.channels())
                .map(|_| fft.make_output_vec())
                .collect(),
        );
        let fft_output_len = fft_output[0].len();
        let mut fft_scratch = fft.make_scratch_vec();

        let (decoder_handle, encoder_handle) = {
            let mut visualizer = program
                .visualizer
                .new_visualizer(VisualizerInputExtra {
                    width: program.width,
                    height: program.height,
                    fft_length: fft_output_len,
                })
                .await
                .context("Creating visualizer")?;

            let (audio_producer, mut audio_consumer) = recycler(
                (0..AUDIO_FRAMES_IN_FLIGHT)
                    .map(|_| frame::Audio::empty())
                    .collect(),
            )
            .await;

            let decoder_handle =
                DecoderHandle::spawn(input_file.clone(), audio_format, audio_producer)
                    .await
                    .context("Spawning decoder handle")?;

            let encoder_state = EncoderState::new(
                output_file.clone(),
                EncoderArgs {
                    in_audio_format: audio_format,
                    width: program.width,
                    height: program.height,
                },
            )
            .await
            .context("Creating encoder")?;

            let (mut video_producer, video_consumer) = enum_recycler(
                (0..AUDIO_FRAMES_IN_FLIGHT)
                    .map(|_| {
                        EncoderFrame::Audio(frame::Audio::new(
                            audio_format.sample_format,
                            audio_format.frame_size.unwrap().get() as usize,
                            audio_format.channel_layout,
                        ))
                    })
                    .chain((0..VIDEO_FRAMES_IN_FLIGHT).map(|_| {
                        EncoderFrame::Video(frame::Video::new(
                            format::Pixel::ARGB,
                            program.width,
                            program.height,
                        ))
                    }))
                    .collect(),
            )
            .await;

            let encoder_handle = encoder_state.spawn(video_consumer);

            let mut last_msg = Instant::now();
            while let Some(mut audio_in) = audio_consumer.recv_data().await {
                {
                    recv_recycling!(video_producer, video_holder, EncoderFrame::Audio(audio_out));

                    if audio_out.samples() > audio_in.samples() {
                        for i in 0..audio_out.planes() {
                            audio_out.plane_mut(i).fill(0f32);
                        }
                    }

                    audio_in.clone_into(audio_out);

                    video_holder.send().await.ok();
                }
                {
                    // TODO: investigate parallelizing this
                    for plane_index in 0..audio_in.planes() {
                        let plane = audio_in.plane::<f32>(plane_index);

                        if plane.len() == fft_input.len() {
                            fft_input.copy_from_slice(plane);
                        } else {
                            fft_input.fill(0.0);
                            fft_input[0..plane.len()].copy_from_slice(plane);
                        }

                        // planar audio should always have channels == planes
                        fft.process_with_scratch(
                            &mut fft_input,
                            &mut fft_output[plane_index],
                            &mut fft_scratch,
                        )
                        .context("Performing Fast Fourier Transform")?;
                    }

                    recv_recycling!(video_producer, video_holder, EncoderFrame::Video(video_out));

                    let video_frame = video_out.data_mut(0);

                    visualizer
                        .render_frame(&audio_in, &fft_output, video_frame)
                        .await
                        .context("Rendering frame")?;

                    video_out.set_pts(audio_in.pts().map(|pts| pts * 24 / 48000));

                    video_holder.send().await.ok();
                }

                let now = Instant::now();
                if now - last_msg > Duration::from_secs(2) {
                    last_msg = now;
                    info!(
                        "Time: {}",
                        humantime::Duration::from(Duration::from_millis(
                            (audio_in.pts().unwrap() / 48) as u64
                        ))
                    );
                }

                audio_in.send().await.ok();
            }

            info!("Closing files...");

            (decoder_handle, encoder_handle)
        };

        encoder_handle
            .join()
            .await
            .context("Waiting for encoder to finish")?;
        decoder_handle
            .join()
            .await
            .context("Waiting for decoder to finish")?;

        info!("Visualization complete.");

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum VisualizeError {
    #[error("No input file specified")]
    NoInputFile,

    #[error("No output file specified")]
    NoOutputFile,
}
