use crate::util::MultiSlice;
use crate::visualizer::{Visualizer, VisualizerInput, VisualizerInputExtra};
use ffmpeg_next::frame::Audio;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use num_complex::Complex32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarsVisualizerInput {}

impl VisualizerInput for BarsVisualizerInput {
    async fn new_visualizer(
        &self,
        extra: VisualizerInputExtra,
    ) -> anyhow::Result<Box<dyn Visualizer>> {
        Ok(Box::new(BarsVisualizer { extra }))
    }
}

pub struct BarsVisualizer {
    extra: VisualizerInputExtra,
}

impl Visualizer for BarsVisualizer {
    fn render_frame<'a>(
        &'a mut self,
        _audio_in: &'a Audio,
        audio_fft: &'a MultiSlice<Complex32>,
        video_out: &'a mut [u8],
    ) -> LocalBoxFuture<'a, anyhow::Result<()>> {
        async {
            // reference implementation
            for x in 0usize..self.extra.width as usize {
                let buf_index = x * self.extra.fft_length / (self.extra.width as usize);

                let pixel_1 = (audio_fft[0][buf_index].norm() * 2.0) as u8;
                let pixel_2 = audio_fft
                    .get(1)
                    .map(|out| (out[buf_index].norm() * 2.0) as u8);

                for y in 0usize..self.extra.height as usize {
                    let pixel = (y * (self.extra.width as usize) + x) * 4;
                    video_out[pixel] = 0xFF; // alpha
                    video_out[pixel + 3] = pixel_1; // blue
                    if let Some(pixel_2) = pixel_2 {
                        video_out[pixel + 2] = pixel_2; // green
                    }
                }
            }

            Ok(())
        }
        .boxed_local()
    }
}
