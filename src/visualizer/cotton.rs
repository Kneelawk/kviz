use crate::util::{MultiSlice, RGB};
use crate::visualizer::{Visualizer, VisualizerInput, VisualizerInputExtra};
use ffmpeg_next::frame::Audio;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use num_complex::Complex32;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CottonVisualizerInput {
    pub seed: Option<u64>,
}

impl VisualizerInput for CottonVisualizerInput {
    async fn new_visualizer(
        &self,
        extra: VisualizerInputExtra,
    ) -> anyhow::Result<Box<dyn Visualizer>> {
        let frame_old = vec![0u8; (extra.width * extra.height * 4) as usize];
        let rand = if let Some(seed) = self.seed {
            SmallRng::seed_from_u64(seed)
        } else {
            SmallRng::from_entropy()
        };

        Ok(Box::new(CottonVisualizer {
            extra,
            rand,
            frame_old,
        }))
    }
}

pub struct CottonVisualizer {
    extra: VisualizerInputExtra,
    rand: SmallRng,
    frame_old: Vec<u8>,
}

impl Visualizer for CottonVisualizer {
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
                    let up_scale: f32 = self.rand.gen();
                    let up_left_scale: f32 = self.rand.gen();
                    let up_right_scale: f32 = self.rand.gen();

                    let mut total = up_scale;

                    let up_left = if x > 0 {
                        total += up_left_scale;
                        RGB::from_pixel(&self.frame_old, x - 1, y - 1, self.extra.width)
                            .scale(up_left_scale)
                    } else {
                        RGB::ZERO
                    };
                    let up_right = if x < self.extra.width as usize - 1 {
                        total += up_right_scale;
                        RGB::from_pixel(&self.frame_old, x + 1, y - 1, self.extra.width)
                            .scale(up_right_scale)
                    } else {
                        RGB::ZERO
                    };
                    let pixel = RGB::from_pixel(&self.frame_old, x, y - 1, self.extra.width)
                        .scale(up_scale)
                        + up_left
                        + up_right;

                    let r_offset = (self.rand.gen::<f32>() - 0.5) * 0.01;
                    let g_offset = (self.rand.gen::<f32>() - 0.5) * 0.01;
                    let b_offset = (self.rand.gen::<f32>() - 0.5) * 0.01;

                    let mut pixel = pixel.scale(1.0 / total);

                    pixel.r += r_offset;
                    pixel.g += g_offset;
                    pixel.b += b_offset;

                    pixel.write_pixel(video_out, x, y, self.extra.width);
                }
            }

            self.frame_old.copy_from_slice(video_out);

            Ok(())
        }
        .boxed_local()
    }
}
