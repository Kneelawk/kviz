use crate::ffmpeg::AudioFormat;
use crate::recycler::RecycleProducer;
use anyhow::Context;
use ffmpeg_next::{codec, filter, format, frame, media, util, Error, Packet, Rational};
use std::path::PathBuf;
use tokio::task::JoinHandle;

pub struct DecoderHandle {
    handle: JoinHandle<anyhow::Result<()>>,
}

impl DecoderHandle {
    pub async fn spawn(
        path: PathBuf,
        output_format: AudioFormat,
        producer: RecycleProducer<frame::Audio>,
    ) -> anyhow::Result<DecoderHandle> {
        let ictx =
            tokio::task::spawn_blocking(move || format::input(&path).context("Opening input file"))
                .await
                .expect("spawn_blocking error")?;

        let (mut ictx, mut state) = tokio::task::spawn_blocking(move || {
            let stream = ictx
                .streams()
                .best(media::Type::Audio)
                .context("No audio stream")?;

            let stream_idx = stream.index();

            let context = codec::context::Context::from_parameters(stream.parameters())
                .context("Initializing input codec")?;
            let mut decoder = context
                .decoder()
                .audio()
                .context("Getting input audio codec")?;

            decoder
                .set_parameters(stream.parameters())
                .context("Setting input codec parameters")?;

            info!("Time base: {}", decoder.time_base());
            info!("Sample rate: {}", decoder.rate());
            info!("Sample format: {:?}", decoder.format());
            info!("Channel Layout: {:?}", decoder.channel_layout());
            info!("Frame size: {}", decoder.frame_size());

            let filter = filter(&decoder, output_format).context("Creating filter graph")?;

            let in_time_base = decoder.time_base();

            Ok::<_, anyhow::Error>((
                ictx,
                DecoderState {
                    producer,
                    stream_idx,
                    filter,
                    decoder,
                    decoded: frame::Audio::empty(),
                    in_time_base,
                },
            ))
        })
        .await
        .expect("spawn_blocking error")
        .context("Creating decoder state")?;

        let handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn_blocking(move || {
            for (stream, mut packet) in ictx.packets() {
                if stream.index() == state.stream_idx {
                    packet.rescale_ts(stream.time_base(), state.in_time_base);
                    state.send_packet_to_decoder(&packet)?;
                    state
                        .receive_and_process_decoded_frames()
                        .context("Decoding frames")?;
                }
            }

            state.send_eof_to_decoder()?;
            state
                .receive_and_process_decoded_frames()
                .context("Decoding final frames")?;

            state.flush_filter()?;
            state
                .get_and_process_filtered_frames()
                .context("Processing final filtered frames")?;

            Ok(())
        });

        Ok(DecoderHandle { handle })
    }

    pub async fn join(self) -> anyhow::Result<()> {
        self.handle.await.expect("join error")
    }
}

struct DecoderState {
    producer: RecycleProducer<frame::Audio>,
    stream_idx: usize,
    filter: filter::Graph,
    decoder: codec::decoder::Audio,
    decoded: frame::Audio,
    in_time_base: Rational,
}

impl DecoderState {
    fn send_packet_to_decoder(&mut self, packet: &Packet) -> anyhow::Result<()> {
        self.decoder
            .send_packet(packet)
            .context("Sending packet to decoder")
    }

    fn send_eof_to_decoder(&mut self) -> anyhow::Result<()> {
        self.decoder.send_eof().context("Sending EOF to decoder")
    }

    fn receive_and_process_decoded_frames(&mut self) -> anyhow::Result<()> {
        while self.decoder.receive_frame(&mut self.decoded).is_ok() {
            let timestamp = self.decoded.timestamp();
            self.decoded.set_pts(timestamp);
            self.add_frame_to_filter()?;
            self.get_and_process_filtered_frames()
                .context("Processing filtered frames")?;
        }

        Ok(())
    }

    fn add_frame_to_filter(&mut self) -> anyhow::Result<()> {
        self.filter
            .get("in")
            .unwrap()
            .source()
            .add(&self.decoded)
            .context("Adding frame to filter")
    }

    fn flush_filter(&mut self) -> anyhow::Result<()> {
        self.filter
            .get("in")
            .unwrap()
            .source()
            .flush()
            .context("Flushing filter")
    }

    fn get_and_process_filtered_frames(&mut self) -> anyhow::Result<()> {
        loop {
            let mut recycling = self
                .producer
                .recv_recycling_blocking()
                .context("Premature recycler drop")?;

            let res = self.filter.get("out").unwrap().sink().frame(&mut recycling);

            match res {
                Err(Error::Other {
                    errno: util::error::EAGAIN,
                }) => return Ok(()),
                Err(Error::Eof) => return Ok(()),
                Err(err) => return Err(err.into()),
                Ok(_) => {}
            }

            recycling.blocking_send().context("Sending frame")?;
        }
    }
}

fn filter(
    decoder: &codec::decoder::Audio,
    output_format: AudioFormat,
) -> anyhow::Result<filter::Graph> {
    let mut filter = filter::Graph::new();

    let mut channel_layout = decoder.channel_layout();
    if channel_layout.is_empty() {
        channel_layout = util::channel_layout::ChannelLayout::default(1);
    }

    let in_args = format!(
        "time_base={}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
        decoder.time_base(),
        decoder.rate(),
        decoder.format().name(),
        channel_layout.bits()
    );

    info!("In args: {}", &in_args);
    info!("Out format: {:?}", output_format);

    filter
        .add(&filter::find("abuffer").unwrap(), "in", &in_args)
        .context("Adding input to filter")?;
    filter
        .add(&filter::find("abuffersink").unwrap(), "out", "")
        .context("Adding output to filter")?;

    {
        let mut out = filter.get("out").unwrap();

        output_format.set(&mut out);
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

    if let Some(frame_size) = output_format.frame_size {
        filter
            .get("out")
            .unwrap()
            .sink()
            .set_frame_size(frame_size.get());
    }

    Ok(filter)
}
