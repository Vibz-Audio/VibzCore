use std::path::Path;
use std::time::Duration;
use symphonia::core::errors::Error;

use crate::wav_decoder::WaveDecoder;

pub struct AudioClip {
    start_offset: Duration,
    end_offset: Duration,
    decoder: WaveDecoder,
}

impl AudioClip {
    pub fn new(
        file_path: &Path,
        start_offset: Option<Duration>,
        end_offset: Option<Duration>,
    ) -> Result<Self, Error> {
        let file = std::fs::File::open(file_path)?;
        let mut decoder = WaveDecoder::try_new(file)?;

        decoder.set_st_time(start_offset.unwrap_or(Duration::from_secs(0)))?;

        Ok(AudioClip {
            start_offset: start_offset.unwrap_or(Duration::from_secs(0)),
            end_offset: end_offset.unwrap_or(Duration::from_secs(0)),
            decoder,
        })
    }

    pub fn decode(&mut self) -> Result<symphonia::core::audio::AudioBufferRef, Error> {
        self.decoder.decode()
    }
}
