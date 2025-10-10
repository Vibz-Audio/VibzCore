use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::decoder::{Decoder, DecoderError};

pub struct WaveDecoder {
    reader: WaveReader,
}

struct WaveReader {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track: Track,
}

impl WaveReader {
    fn new(
        format: Box<dyn symphonia::core::formats::FormatReader>,
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
        track: Track,
    ) -> Self {
        WaveReader {
            format,
            decoder,
            track,
        }
    }
}

impl WaveDecoder {
    pub fn try_new(file: std::fs::File) -> Result<WaveDecoder, Error> {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(
            Hint::new().with_extension("wav"),
            mss,
            &fmt_opts,
            &meta_opts,
        )?;

        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(Error::DecodeError("No supported audio tracks"))?
            .clone();

        let dec_opts: DecoderOptions = Default::default();
        let decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;
        let reader = WaveReader::new(format, decoder, track);

        Ok(WaveDecoder { reader })
    }
}

impl Decoder for WaveDecoder {
    //TODO: Fix errors
    fn decode(&mut self) -> Result<Vec<f32>, DecoderError> {
        let packet = self.reader.format.next_packet().unwrap();
        let packet = self.reader.decoder.decode(&packet).unwrap();
        let mut buffer = SampleBuffer::new(packet.capacity() as u64, *packet.spec());

        buffer.copy_interleaved_ref(packet);

        Ok(buffer.samples().to_vec())
    }

    fn seek(&mut self, start_t: Duration) -> Result<(), DecoderError> {
        let track = &self.reader.track;

        self.reader
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::from(start_t),
                    track_id: Some(track.id),
                },
            )
            .expect("Failed to seek");

        Ok(())
    }
}
