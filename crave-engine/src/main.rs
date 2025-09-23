#![allow(unused_variables)]
mod buffer_config;
mod clip;
mod playback;
mod wav_decoder;

use ringbuf::traits::{Observer, Producer, Split};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;

enum ProcessingControlMessage {
    RequestData,
    DecodingDone,
    BufferFull,
    BufferRecharge,
    BufferUnderrun,
}

pub enum AudioProducerMessage {
    RequestData,
}

fn main() {
    let b_config = buffer_config::AudioBufferConfig::default();

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

    let mut all_clips: Vec<clip::AudioClip> = clip_paths
        .iter()
        .map(|path| {
            clip::AudioClip::new(Path::new(path), Some(Duration::from_millis(00_000)), None)
                .expect("Failed to create audio clip")
        })
        .collect();

    let rb = b_config.create_ring_buffer();
    let (mut producer, consumer) = rb.split();

    // Channels for communication
    let (tx_playback, rx_playback) = mpsc::channel::<AudioProducerMessage>();
    let (tx_control, rx_control) = mpsc::channel::<ProcessingControlMessage>();
    let tx_control_worker = tx_control.clone();

    // Prevent buffer underrun at start
    tx_playback.send(AudioProducerMessage::RequestData).unwrap();

    let audio_done = Arc::new(AtomicBool::new(false));
    let audio_done_worker = audio_done.clone();

    let (audio_player, command_sender) =
        playback::AudioPlayer::new(consumer, tx_playback.clone(), b_config.clone());
    let stream = audio_player.create_stream();

    // Setup non-blocking stdin for CLI commands
    let (cli_tx, cli_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(input) = line {
                if cli_tx.send(input).is_err() {
                    break;
                }
            }
        }
    });

    /* Worker thread for decoding and buffering */
    let mut sample_buf = all_clips
        .iter()
        .map(|_| None)
        .collect::<Vec<Option<SampleBuffer<f32>>>>();
    thread::spawn(move || {
        'outer: loop {
            if let Ok(msg) = rx_playback.try_recv() {
                loop {
                    if producer.occupied_len() > b_config.threshold {
                        tx_control_worker
                            .send(ProcessingControlMessage::BufferFull)
                            .ok();
                        break;
                    }
                    let mut mixed_samples: Vec<f32> = Vec::new();
                    let mut any_decoded = false;

                    // Decode from all clips and mix them together
                    for (idx, clip) in all_clips.iter_mut().enumerate() {
                        match clip.decode() {
                            Ok(_decoded) => {
                                any_decoded = true;
                                if sample_buf[idx].is_none() {
                                    let spec = *_decoded.spec();
                                    let duration = _decoded.capacity() as u64;
                                    sample_buf[idx] =
                                        Some(SampleBuffer::<f32>::new(duration, spec));
                                }
                                if let Some(buf) = &mut sample_buf[idx] {
                                    buf.copy_interleaved_ref(_decoded);
                                    let samples = buf.samples();

                                    // We never wrote into the buffer
                                    if mixed_samples.is_empty() {
                                        mixed_samples = samples.to_vec();
                                    } else {
                                        // Mixing
                                        for (i, &sample) in samples.iter().enumerate() {
                                            if i < mixed_samples.len() {
                                                mixed_samples[i] += sample / 2.0;
                                            } else {
                                                // In case clips are of different lengths
                                                mixed_samples.push(sample);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(Error::ResetRequired) => {
                                println!("Decoder reset required for a clip");
                                break;
                            }
                            Err(_) => {
                                // This clip is done, continue with other clips
                            }
                        }
                    }

                    // Push mixed audio to ring buffer if we have any samples
                    if any_decoded && !mixed_samples.is_empty() {
                        tx_control_worker
                            .send(ProcessingControlMessage::BufferRecharge)
                            .ok();
                        producer.push_slice(&mixed_samples);
                    } else {
                        // All clips are done
                        tx_control_worker
                            .send(ProcessingControlMessage::DecodingDone)
                            .ok();
                        if producer.is_empty() {
                            audio_done_worker.store(true, std::sync::atomic::Ordering::Relaxed);
                            break 'outer;
                        }
                    }
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });

    println!("🎮 Playback Controls:");
    println!("  'p' - Play/Pause toggle");

    while !audio_done.load(std::sync::atomic::Ordering::Relaxed) {
        audio_player.process_commands();

        // Check for CLI commands (non-blocking)
        if let Ok(input) = cli_rx.try_recv() {
            match input.trim() {
                "p" => {
                    if command_sender
                        .send(playback::AudioPlayerCommand::TogglePlayPause)
                        .is_err()
                    {
                        break;
                    }
                }
                _ => {
                    println!(
                        "Unknown command: '{}'. Use 'p' to toggle playback.",
                        input.trim()
                    );
                }
            }
        }

        // Check for processing control messages
        match rx_control.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(msg) => match msg {
                ProcessingControlMessage::DecodingDone => {
                    println!("✅ Decoding done");
                }
                ProcessingControlMessage::BufferFull => {
                    println!("🔋 Buffer full");
                }
                ProcessingControlMessage::RequestData => {
                    println!("🪫 Requesting more data");
                }
                ProcessingControlMessage::BufferUnderrun => {
                    println!("⚠️ Buffer underrun, audio may stutter");
                }
                ProcessingControlMessage::BufferRecharge => {}
            },
            Err(_) => {}
        }
    }
}
