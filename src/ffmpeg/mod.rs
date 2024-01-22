use anyhow::Context;
use ffmpeg_next::{codec, filter, format, util};
use std::num::NonZeroU32;

pub mod decode;
mod logging;

pub fn init_ffmpeg() -> anyhow::Result<()> {
    ffmpeg_next::init().context("Initializing ffmpeg_next")?;

    logging::setup_logging();

    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    sample_format: format::Sample,
    channel_layout: util::channel_layout::ChannelLayout,
    sample_rate: u32,
    pub frame_size: Option<NonZeroU32>,
}

impl Default for AudioFormat {
    fn default() -> Self {
        AudioFormat {
            sample_format: format::Sample::F32(format::sample::Type::Planar),
            channel_layout: util::channel_layout::ChannelLayout::default(2),
            sample_rate: 48000,
            frame_size: None,
        }
    }
}

impl AudioFormat {
    pub fn from_encoder(encoder: &codec::encoder::Audio) -> AudioFormat {
        let frame_size = encoder
            .codec()
            .filter(|codec| {
                !codec
                    .capabilities()
                    .contains(codec::capabilities::Capabilities::VARIABLE_FRAME_SIZE)
            })
            .and_then(|_codec| NonZeroU32::new(encoder.frame_size()));

        AudioFormat {
            sample_format: encoder.format(),
            channel_layout: encoder.channel_layout(),
            sample_rate: encoder.rate(),
            frame_size,
        }
    }

    pub fn set(&self, ctx: &mut filter::Context) {
        ctx.set_sample_format(self.sample_format);
        ctx.set_channel_layout(self.channel_layout);
        ctx.set_sample_rate(self.sample_rate);
    }

    pub fn format(&self) -> String {
        format!(
            "sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
            self.sample_rate,
            self.sample_format.name(),
            self.channel_layout.bits()
        )
    }
}
