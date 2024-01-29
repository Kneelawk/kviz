//! This module contains the different visualizer modules

use crate::util::MultiSlice;
use ffmpeg_next::frame;
use futures::future::LocalBoxFuture;
use num_complex::Complex32;

pub mod bars;

#[derive(Debug, Clone)]
pub struct VisualizerInputExtra {
    pub width: u32,
    pub height: u32,
    pub fft_length: usize,
}

pub trait VisualizerInput {
    async fn new_visualizer(
        &self,
        extra: VisualizerInputExtra,
    ) -> anyhow::Result<Box<dyn Visualizer>>;
}

pub trait Visualizer {
    fn render_frame<'a>(
        &'a mut self,
        audio_in: &'a frame::Audio,
        audio_fft: &'a MultiSlice<Complex32>,
        video_out: &'a mut [u8],
    ) -> LocalBoxFuture<'a, anyhow::Result<()>>;
}
