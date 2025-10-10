#![allow(unused_variables)]
mod audio_config;
mod clip;
mod decoder;
mod messages;
mod playback;
mod producer;
mod visualizer;
mod wav_decoder;

use ringbuf::traits::Split;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::messages::{AudioOutput, PlayerCommand, ProducerCommand, ProducerStatus};
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

    let audio_done = Arc::new(AtomicBool::new(false));
    let audio_done_worker = audio_done.clone();

    let (mut audio_player, playback_command_tx) =
        playback::AudioPlayer::new(consumer, producer_request_tx.clone(), audio_config.clone());

    // Create audio output channel for message passing
    let (audio_output_tx, audio_output_rx) = crossbeam::channel::unbounded::<AudioOutput>();
    audio_player.set_output_channel(audio_output_tx);

    // Create and set up the visualizer
    let visualizer = Arc::new(visualizer::AudioVisualizer::new(1024, 80));

    let stream = audio_player.create_stream();

    // Setup non-blocking stdin for CLI commands
    let (cli_input_tx, cli_input_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(input) = line {
                if cli_input_tx.send(input).is_err() {
                    break;
                }
            }
        }
    });

    /* Start audio producer */
    let audio_producer = producer::AudioProducer::new(clips, producer, audio_config.clone());
    audio_producer.start_production(
        producer_request_rx,
        producer_status_tx_clone,
        audio_done_worker,
    );

    // Start visualization thread that listens to audio output messages
    let visualizer_for_display = visualizer.clone();
    let audio_done_for_viz = audio_done.clone();
    thread::spawn(move || {
        while !audio_done_for_viz.load(std::sync::atomic::Ordering::Relaxed) {
            // Process audio output messages
            while let Ok(audio_output) = audio_output_rx.try_recv() {
                for sample in audio_output.samples {
                    visualizer_for_display.add_sample(sample);
                }
            }

            visualizer::display_visualization(&visualizer_for_display);
            thread::sleep(Duration::from_millis(50)); // 20 FPS
        }
    });

    // Initial setup complete - visualization will display controls

    while !audio_done.load(std::sync::atomic::Ordering::Relaxed) {
        audio_player.process_commands();

        // Check for CLI commands (non-blocking)
        if let Ok(input) = cli_input_rx.try_recv() {
            match input.trim() {
                "p" => {
                    if playback_command_tx
                        .send(PlayerCommand::TogglePlayPause)
                        .is_err()
                    {
                        break;
                    }
                }
                "q" => {
                    break;
                }
                _ => {
                    // Silently ignore unknown commands to avoid interfering with visualization
                }
            }
        }

        // Check for producer status messages (but don't print to avoid interfering with visualization)
        match producer_status_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(msg) => match msg {
                ProducerStatus::DecodingDone => {
                    // Silently handle completion
                }
                ProducerStatus::BufferFull => {}
                ProducerStatus::RequestData => {}
                ProducerStatus::BufferUnderrun => {}
                ProducerStatus::BufferRecharge => {}
            },
            Err(_) => {}
        }
    }
}
