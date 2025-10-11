/// Commands sent to the audio producer to request data production
#[derive(Debug, Clone)]
pub enum ProducerCommand {
    RequestData,
}

/// Status updates sent from the audio producer to the main thread
#[derive(Debug, Clone)]
pub enum ProducerStatus {
    RequestData,
    DecodingDone,
    BufferFull,
    BufferRecharge,
    BufferUnderrun,
}

/// Commands sent to the audio player for playback control
#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play,
    Pause,
    TogglePlayPause,
}



