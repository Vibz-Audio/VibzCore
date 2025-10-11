#![allow(unused_variables)]
mod audio_config;
mod clip;
mod decoder;
mod messages;
mod playback;
mod producer;
mod wav_decoder;

use ringbuf::traits::Split;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::messages::{ProducerCommand, ProducerStatus};
use crate::wav_decoder::WaveDecoder;

fn main() {
    let audio_config = audio_config::AudioBufferConfig::default();

    let clip_paths = vec![
        // "samples/trackouts/dnb/01_Drumloop1.wav",
        // "samples/trackouts/dnb/02_Drumloop2.wav",
        // "samples/trackouts/dnb/03_Kick1.wav",
        // "samples/trackouts/dnb/04_Kick2.wav",
        // "samples/trackouts/dnb/05_Snare.wav",
        // "samples/trackouts/dnb/06_Ride1.wav",
        // "samples/trackouts/dnb/07_Ride2.wav",
        // "samples/trackouts/dnb/08_HiHat.wav",
        // "samples/trackouts/dnb/09_SFX1.wav",
        // "samples/trackouts/dnb/10_SFX2.wav",
        // "samples/trackouts/dnb/11_Bass1.wav",
        // "samples/trackouts/dnb/12_Bass2.wav",
        // "samples/trackouts/dnb/13_BassSub.wav",
        // "samples/trackouts/dnb/14_Strings1.wav",
        // "samples/trackouts/dnb/15_Strings2.wav",
        "samples/crave.wav",
    ];

    let clips: Vec<clip::AudioClip<WaveDecoder>> = clip_paths
        .iter()
        .map(|path| {
            clip::AudioClip::new(Path::new(path), Some(Duration::from_millis(30_000)), None)
                .expect("Failed to create audio clip")
        })
        .collect();

    let rb = audio_config.create_ring_buffer();
    let (producer, consumer) = rb.split();

    // Channels for communication
    let (producer_request_tx, producer_request_rx) = mpsc::channel::<ProducerCommand>();
    let (producer_status_tx, producer_status_rx) = mpsc::channel::<ProducerStatus>();
    let producer_status_tx_clone = producer_status_tx.clone();

    // Prevent buffer underrun at start
    producer_request_tx
        .send(ProducerCommand::RequestData)
        .unwrap();

    let (audio_player, playback_command_tx) =
        playback::AudioPlayer::new(consumer, producer_request_tx.clone(), audio_config.clone());

    let stream = audio_player.create_stream();

    /* Start audio producer */
    let audio_producer = producer::AudioProducer::new(clips, producer, audio_config.clone());
    audio_producer.start_production(producer_request_rx, producer_status_tx_clone);

    loop {}
}
