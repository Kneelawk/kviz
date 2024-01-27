use crate::ffmpeg::{audio_filter, AudioFormat, FfmpegResult};
use crate::recycle::simple::RecycleProducer;
use anyhow::Context;
use ffmpeg_next::format::context::Input;
use ffmpeg_next::{codec, filter, format, frame, media, Packet, Rational};
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
        let (ictx, state) = tokio::task::spawn_blocking(move || {
            let ictx = format::input(&path).context("Opening input file")?;

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

            format::context::input::dump(&ictx, 0, Some(&path.to_string_lossy()));

            info!("Time base: {}", decoder.time_base());
            info!("Sample rate: {}", decoder.rate());
            info!("Sample format: {:?}", decoder.format());
            info!("Channel Layout: {:?}", decoder.channel_layout());
            info!("Frame size: {}", decoder.frame_size());

            let filter = audio_filter(AudioFormat::from_decoder(&decoder), output_format)
                .context("Creating filter graph")?;

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

        let handle: JoinHandle<anyhow::Result<()>> =
            tokio::task::spawn_blocking(move || match Self::do_decode(ictx, state) {
                Ok(()) => Ok(()),
                Err(err) => {
                    error!("Decode error: {:#}", &err);
                    Err(err)
                }
            });

        Ok(DecoderHandle { handle })
    }

    fn do_decode(mut ictx: Input, mut state: DecoderState) -> anyhow::Result<()> {
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

        info!("Done decoding.");

        Ok(())
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
        while self
            .decoder
            .receive_frame(&mut self.decoded)
            .recv_continue()
            .context("Receive frame from audio decoder")?
        {
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

            if !self
                .filter
                .get("out")
                .unwrap()
                .sink()
                .frame(&mut recycling)
                .recv_continue()
                .context("Receiving audio frame from filter")?
            {
                return Ok(());
            }

            recycling.blocking_send().context("Sending frame")?;
        }
    }
}
