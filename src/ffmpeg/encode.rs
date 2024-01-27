use crate::ffmpeg::extra::SourceExtra;
use crate::ffmpeg::{audio_filter, AudioFormat, FfmpegResult};
use crate::recycle::r#enum::EnumRecycleConsumer;
use anyhow::Context;
use enum_key::KeyableEnum;
use ffmpeg_next::{codec, encoder, filter, format, frame, software, Dictionary, Packet, Rational};
use std::num::NonZeroU32;
use std::ops::Deref;
use std::path::PathBuf;
use tokio::task::JoinHandle;

#[derive(KeyableEnum)]
pub enum EncoderFrame {
    Audio(frame::Audio),
    Video(frame::Video),
}

pub struct EncoderState {
    octx: format::context::Output,
    aidx: usize,
    vidx: usize,
    audio_filter: filter::Graph,
    audio_encoder: encoder::Audio,
    video_encoder: encoder::Video,
    in_audio_tb: Rational,
    out_audio_tb: Rational,
    in_video_tb: Rational,
    out_video_tb: Rational,
    audio_filtered: frame::Audio,
    packet: Packet,
}

impl EncoderState {
    pub async fn new(path: PathBuf, audio_format: AudioFormat) -> anyhow::Result<EncoderState> {
        tokio::task::spawn_blocking(move || {
            let mut octx = format::output(&path).context("Opening output file")?;

            let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);

            let (video_encoder, vidx, out_video_tb) = {
                let mut vost = octx
                    .add_stream(encoder::find(codec::Id::VP9))
                    .context("Adding video stream")?;
                let mut video_encoder = codec::context::Context::from_parameters(vost.parameters())
                    .context("Getting video context")?
                    .encoder()
                    .video()
                    .context("Getting video encoder")?;

                video_encoder.set_width(1920);
                video_encoder.set_height(1080);
                video_encoder.set_frame_rate(Some((1, 24)));
                video_encoder.set_time_base((1, 24));
                video_encoder.set_format(format::Pixel::YUV420P);
                video_encoder.set_bit_rate(0);

                vost.set_time_base((1, 24));

                if global_header {
                    video_encoder.set_flags(codec::Flags::GLOBAL_HEADER);
                }

                let mut dict = Dictionary::new();
                dict.set("crf", "30");

                let video_encoder = video_encoder
                    .open_as_with(encoder::find(codec::Id::VP9), dict)
                    .context("Opening video encoder")?;

                vost.set_parameters(&video_encoder);

                (video_encoder, vost.index(), vost.time_base())
            };

            let output_audio_format = AudioFormat {
                time_base: Some(Rational::new(1, 48000)),
                sample_format: format::Sample::F32(format::sample::Type::Packed),
                channel_layout: audio_format.channel_layout,
                sample_rate: 48000,
                frame_size: NonZeroU32::new(960),
            };

            let (audio_encoder, aidx, out_audio_tb) = {
                let mut aost = octx
                    .add_stream(encoder::find(codec::Id::OPUS))
                    .context("Adding audio stream")?;
                let mut audio_encoder = codec::context::Context::from_parameters(aost.parameters())
                    .context("Getting audio context")?
                    .encoder()
                    .audio()
                    .context("Getting audio encoder")?;

                audio_encoder.set_channel_layout(output_audio_format.channel_layout);
                audio_encoder.set_format(output_audio_format.sample_format);
                audio_encoder.set_rate(output_audio_format.sample_rate as i32);
                audio_encoder.set_time_base(output_audio_format.time_base.unwrap());
                aost.set_time_base(output_audio_format.time_base.unwrap());

                if global_header {
                    audio_encoder.set_flags(codec::Flags::GLOBAL_HEADER);
                }

                let audio_encoder = audio_encoder
                    .open_as(encoder::find(codec::Id::OPUS))
                    .context("Opening audio encoder")?;
                aost.set_parameters(&audio_encoder);

                (audio_encoder, aost.index(), aost.time_base())
            };

            format::context::output::dump(&octx, 0, Some(&path.to_string_lossy()));

            let audio_filter = audio_filter(audio_format, output_audio_format)
                .context("Creating encoder audio filter")?;

            Ok::<_, anyhow::Error>(EncoderState {
                octx,
                aidx,
                vidx,
                audio_filter,
                audio_encoder,
                video_encoder,
                in_audio_tb: Rational::new(1, 48000),
                out_audio_tb,
                in_video_tb: Rational::new(1, 24),
                out_video_tb,
                audio_filtered: frame::Audio::empty(),
                packet: Packet::empty(),
            })
        })
        .await
        .expect("spawn_blocking error")
    }

    pub fn spawn(self, consumer: EnumRecycleConsumer<EncoderFrame>) -> EncoderHandle {
        let handle = tokio::task::spawn_blocking(move || match self.do_encode(consumer) {
            Ok(()) => Ok(()),
            Err(err) => {
                error!("Encode error: {:#}", &err);
                Err(err)
            }
        });

        EncoderHandle { handle }
    }

    fn do_encode(mut self, mut consumer: EnumRecycleConsumer<EncoderFrame>) -> anyhow::Result<()> {
        // The converter cannot be sent between threads, so it needs to be constructed here
        let mut video_converter =
            software::converter((1920, 1080), format::Pixel::ARGB, format::Pixel::YUV420P)
                .context("Creating video converter")?;
        let mut video_converted = frame::Video::empty();

        self.octx.write_header().context("Writing header")?;

        while let Some(mut frame) = consumer.recv_data_blocking() {
            match frame.deref() {
                EncoderFrame::Audio(audio) => {
                    info!("Encoding frame: {:?}", audio);
                    self.send_frame_to_audio_filter(audio)?;
                    self.receive_and_process_filtered_frames()
                        .context("Processing filtered audio")?;
                }
                EncoderFrame::Video(video) => {
                    video_converter
                        .run(video, &mut video_converted)
                        .context("Converting video frame")?;
                    video_converted.set_pts(video.pts());
                    self.send_frame_to_video_encoder(&video_converted)?;
                    self.receive_and_process_encoded_video()
                        .context("Processing encoded video")?;
                }
            }

            frame.blocking_send().ok();
        }

        info!("Finishing up encoding...");

        self.flush_audio_filter()?;
        self.receive_and_process_filtered_frames()
            .context("Processing final filtered audio")?;

        self.send_eof_to_audio_encoder()?;
        self.receive_and_process_encoded_audio()
            .context("Processing encoded EOF audio")?;

        self.send_eof_to_video_encoder()?;
        self.receive_and_process_encoded_video()
            .context("Processing encoded EOF video")?;

        self.octx.write_trailer().context("Writing trailer")?;

        info!("Encoding done.");

        Ok(())
    }

    fn send_frame_to_audio_filter(&mut self, audio: &frame::Audio) -> anyhow::Result<()> {
        // use custom 'write' function to work around ffmpeg taking ownership of our recycled frames
        self.audio_filter
            .get("in")
            .unwrap()
            .write(audio)
            .context("Adding frame to audio filter")
    }

    fn flush_audio_filter(&mut self) -> anyhow::Result<()> {
        self.audio_filter
            .get("in")
            .unwrap()
            .source()
            .flush()
            .context("Flushing audio filter")
    }

    fn receive_and_process_filtered_frames(&mut self) -> anyhow::Result<()> {
        while self
            .audio_filter
            .get("out")
            .unwrap()
            .sink()
            .frame(&mut self.audio_filtered)
            .recv_continue()
            .context("Receiving audio frame from filter")?
        {
            self.send_frame_to_audio_encoder()?;
            self.receive_and_process_encoded_audio()
                .context("Processing audio packets")?;
        }

        Ok(())
    }

    fn send_frame_to_audio_encoder(&mut self) -> anyhow::Result<()> {
        self.audio_encoder
            .send_frame(&self.audio_filtered)
            .context("Sending audio frame to encoder")
    }

    fn send_eof_to_audio_encoder(&mut self) -> anyhow::Result<()> {
        self.audio_encoder
            .send_eof()
            .context("Sending EOF to audio encoder")
    }

    fn receive_and_process_encoded_audio(&mut self) -> anyhow::Result<()> {
        while self
            .audio_encoder
            .receive_packet(&mut self.packet)
            .recv_continue()
            .context("Receive packet from audio encoder")?
        {
            self.packet.set_stream(self.aidx);
            self.packet.rescale_ts(
                self.in_audio_tb,
                self.octx.stream(self.aidx).unwrap().time_base(),
            );
            self.packet
                .write_interleaved(&mut self.octx)
                .context("Writing audio packet")?;
        }

        Ok(())
    }

    fn send_frame_to_video_encoder(&mut self, video: &frame::Video) -> anyhow::Result<()> {
        self.video_encoder
            .send_frame(video)
            .context("Sending video frame to encoder")
    }

    fn send_eof_to_video_encoder(&mut self) -> anyhow::Result<()> {
        self.video_encoder
            .send_eof()
            .context("Sending EOF to video encoder")
    }

    fn receive_and_process_encoded_video(&mut self) -> anyhow::Result<()> {
        while self
            .video_encoder
            .receive_packet(&mut self.packet)
            .recv_continue()
            .context("Receive packet from video encoder")?
        {
            self.packet.set_stream(self.vidx);
            self.packet.rescale_ts(
                self.in_video_tb,
                self.octx.stream(self.vidx).unwrap().time_base(),
            );
            self.packet
                .write_interleaved(&mut self.octx)
                .context("Writing video packet")?;
        }

        Ok(())
    }
}

pub struct EncoderHandle {
    handle: JoinHandle<anyhow::Result<()>>,
}

impl EncoderHandle {
    pub async fn join(self) -> anyhow::Result<()> {
        self.handle.await.expect("join error")
    }
}
