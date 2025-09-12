use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct WaveDecoder {
    reader: WaveReader,
}

struct WaveReader {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
}

impl WaveReader {
    fn new(
        format: Box<dyn symphonia::core::formats::FormatReader>,
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
    ) -> Self {
        WaveReader { format, decoder }
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
            .ok_or(Error::DecodeError("No supported audio tracks"))?;

        let dec_opts: DecoderOptions = Default::default();
        let decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

        let reader = WaveReader::new(format, decoder);

        Ok(WaveDecoder { reader })
    }

    pub fn decode(&mut self) -> Result<AudioBufferRef, Error> {
        let packet = self.reader.format.next_packet()?;
        self.reader.decoder.decode(&packet)
    }
}
