use crate::util::{pixel, MultiSlice};
use crate::visualizer::{Visualizer, VisualizerInput, VisualizerInputExtra};
use ffmpeg_next::frame::Audio;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use num_complex::Complex32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditsVisualizerInput {}

impl VisualizerInput for CreditsVisualizerInput {
    async fn new_visualizer(
        &self,
        extra: VisualizerInputExtra,
    ) -> anyhow::Result<Box<dyn Visualizer>> {
        let frame_old = vec![0u8; (extra.width * extra.height * 4) as usize];

        Ok(Box::new(CreditsVisualizer { extra, frame_old }))
    }
}

pub struct CreditsVisualizer {
    extra: VisualizerInputExtra,
    frame_old: Vec<u8>,
}

impl Visualizer for CreditsVisualizer {
    fn render_frame<'a>(
        &'a mut self,
        _audio_in: &'a Audio,
        audio_fft: &'a MultiSlice<Complex32>,
        video_out: &'a mut [u8],
    ) -> LocalBoxFuture<'a, anyhow::Result<()>> {
        async {
            for i in 0usize..(self.extra.width as usize / 2) {
                let buf_index = i * self.extra.fft_length / (self.extra.width as usize / 2);

                let b = (audio_fft[0][buf_index].norm() * 2.0) as u8;
                let g = audio_fft
                    .get(1)
                    .map(|out| (out[buf_index].norm() * 2.0) as u8);

                let x1 = (self.extra.width as usize / 2) + i;
                let x2 = (self.extra.width as usize / 2) - i - 1;

                let pixel_1 = x1 * 4;
                let pixel_2 = x2 * 4;

                video_out[pixel_1] = 0xFF; // alpha
                video_out[pixel_1 + 3] = b; // blue
                if let Some(g) = g {
                    video_out[pixel_1 + 2] = g; // green
                }

                video_out[pixel_2] = 0xFF; // alpha
                video_out[pixel_2 + 3] = b; // blue
                if let Some(g) = g {
                    video_out[pixel_2 + 2] = g; // green
                }
            }

            for y in 1usize..(self.extra.height as usize) {
                for x in 0usize..(self.extra.width as usize) {
                    let pixel_up = self.get_old_pixel(x, y - 1);

                    let index = pixel(x, y, self.extra.width);
                    video_out[index] = 0xFF;
                    video_out[index + 1] = pixel_up.0;
                    video_out[index + 2] = pixel_up.1;
                    video_out[index + 3] = pixel_up.2;
                }
            }

            self.frame_old.copy_from_slice(video_out);

            Ok(())
        }
        .boxed_local()
    }
}

impl CreditsVisualizer {
    fn get_old_pixel(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let index = pixel(x, y, self.extra.width);
        (
            self.frame_old[index + 1],
            self.frame_old[index + 2],
            self.frame_old[index + 3],
        )
    }
}
