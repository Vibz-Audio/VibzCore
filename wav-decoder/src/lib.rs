use std::time::Duration;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

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

// TODO: Implement A WaveDecoderOptions struct to configure the decoder
// in order to make the WaveDecoder more flexible and reuse logic
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

    pub fn set_st_time(&mut self, start_t: Duration) -> Result<(), Error> {
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

    pub fn decode(&mut self) -> Result<AudioBufferRef, Error> {
        let packet = self.reader.format.next_packet()?;
        self.reader.decoder.decode(&packet)
    }
}
