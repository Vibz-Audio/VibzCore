use std::path::Path;
use std::time::Duration;
use symphonia::core::errors::Error;

use crate::decoder::{Decoder, DecoderError};
use crate::wav_decoder::WaveDecoder;

pub struct AudioClip<T>
where
    T: Decoder,
{
    start_offset: Duration,
    end_offset: Duration,
    decoder: T,
}

impl AudioClip<WaveDecoder> {
    pub fn new(
        file_path: &Path,
        start_offset: Option<Duration>,
        end_offset: Option<Duration>,
    ) -> Result<Self, Error> {
        let file = std::fs::File::open(file_path)?;
        let mut decoder = WaveDecoder::try_new(file).unwrap();

        decoder
            .seek(start_offset.unwrap_or(Duration::from_secs(0)))
            .unwrap();

        Ok(AudioClip {
            start_offset: start_offset.unwrap_or(Duration::from_secs(0)),
            end_offset: end_offset.unwrap_or(Duration::from_secs(0)),
            decoder,
        })
    }
}

impl<T> AudioClip<T>
where
    T: Decoder,
{
    pub fn decode(&mut self) -> Result<Vec<f32>, DecoderError> {
        self.decoder.decode()
    }
}
