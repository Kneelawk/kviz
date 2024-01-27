use anyhow::Context;
use ffmpeg_next::{codec, filter, format, frame, util, Error, Rational};
use std::num::NonZeroU32;

pub mod decode;
pub mod encode;
mod logging;
pub mod extra;

pub fn init_ffmpeg() -> anyhow::Result<()> {
    ffmpeg_next::init().context("Initializing ffmpeg_next")?;

    logging::setup_logging();

    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    pub time_base: Option<Rational>,
    pub sample_format: format::Sample,
    pub channel_layout: util::channel_layout::ChannelLayout,
    pub sample_rate: u32,
    pub frame_size: Option<NonZeroU32>,
}

impl Default for AudioFormat {
    fn default() -> Self {
        AudioFormat {
            time_base: Some(Rational::new(1, 48000)),
            sample_format: format::Sample::F32(format::sample::Type::Planar),
            channel_layout: util::channel_layout::ChannelLayout::default(2),
            sample_rate: 48000,
            frame_size: NonZeroU32::new(2000),
        }
    }
}

impl AudioFormat {
    pub fn from_decoder(decoder: &codec::decoder::Audio) -> AudioFormat {
        let mut channel_layout = decoder.channel_layout();
        if channel_layout.is_empty() {
            channel_layout = util::channel_layout::ChannelLayout::default(1);
        }

        AudioFormat {
            time_base: Some(decoder.time_base()),
            sample_format: decoder.format(),
            channel_layout,
            sample_rate: decoder.rate(),
            frame_size: None,
        }
    }

    pub fn from_encoder(encoder: &codec::encoder::Audio) -> AudioFormat {
        let frame_size = encoder
            .codec()
            .filter(|codec| {
                !codec
                    .capabilities()
                    .contains(codec::capabilities::Capabilities::VARIABLE_FRAME_SIZE)
            })
            .and_then(|_codec| NonZeroU32::new(encoder.frame_size()));

        let mut channel_layout = encoder.channel_layout();
        if channel_layout.is_empty() {
            channel_layout = util::channel_layout::ChannelLayout::default(1);
        }

        AudioFormat {
            time_base: None,
            sample_format: encoder.format(),
            channel_layout,
            sample_rate: encoder.rate(),
            frame_size,
        }
    }

    pub fn from_frame(frame: &frame::Audio) -> AudioFormat {
        AudioFormat {
            time_base: None,
            sample_format: frame.format(),
            channel_layout: frame.channel_layout(),
            sample_rate: frame.rate(),
            frame_size: None,
        }
    }

    pub fn output_pre_set(&self, ctx: &mut filter::Context) {
        ctx.set_sample_format(self.sample_format);
        ctx.set_channel_layout(self.channel_layout);
        ctx.set_sample_rate(self.sample_rate);
    }

    pub fn output_post_set<'a, 'b: 'a>(&self, ctx: &'b mut filter::Context<'a>) {
        if let Some(frame_size) = self.frame_size {
            ctx.sink().set_frame_size(frame_size.get());
        }
    }

    pub fn input_format(&self) -> String {
        let time_base = if let Some(time_base) = self.time_base {
            format!("time_base={}:", time_base)
        } else {
            "".to_string()
        };
        format!(
            "{}sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
            time_base,
            self.sample_rate,
            self.sample_format.name(),
            self.channel_layout.bits()
        )
    }
}

impl From<AudioFormat> for (format::Sample, util::channel_layout::ChannelLayout, u32) {
    fn from(value: AudioFormat) -> Self {
        (value.sample_format, value.channel_layout, value.sample_rate)
    }
}

pub fn audio_filter(
    input_format: AudioFormat,
    output_format: AudioFormat,
) -> anyhow::Result<filter::Graph> {
    let mut filter = filter::Graph::new();

    let in_args = input_format.input_format();

    filter
        .add(&filter::find("abuffer").unwrap(), "in", &in_args)
        .context("Adding input to filter")?;
    filter
        .add(&filter::find("abuffersink").unwrap(), "out", "")
        .context("Adding output to filter")?;

    {
        let mut out = filter.get("out").unwrap();
        output_format.output_pre_set(&mut out);
    }

    filter
        .output("in", 0)
        .context("Setting input")?
        .input("out", 0)
        .context("Setting output")?
        .parse("anull")
        .context("Setting filter spec")?;

    info!("Filter:\n{}", filter.dump());

    filter.validate().context("Validating filter")?;

    {
        let mut out = filter.get("out").unwrap();
        output_format.output_post_set(&mut out);
    }

    Ok(filter)
}

pub trait FfmpegResult {
    fn recv_continue(self) -> Result<bool, Error>;
}

impl FfmpegResult for Result<(), Error> {
    fn recv_continue(self) -> Result<bool, Error> {
        match self {
            Err(Error::Other {
                errno: util::error::EAGAIN,
            }) => Ok(false),
            Err(Error::Eof) => Ok(false),
            Err(err) => Err(err),
            Ok(_) => Ok(true),
        }
    }
}
