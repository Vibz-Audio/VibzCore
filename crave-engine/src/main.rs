#![allow(unused_variables)]
mod buffer_config;
mod clip;
mod wav_decoder;

use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
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

enum PlaybackMessage {
    RequestData,
}

fn main() {
    let b_config = buffer_config::AudioBufferConfig::default();

    let clip_paths = vec![
        "samples/trackouts/dnb/01_Drumloop1.wav",
        "samples/trackouts/dnb/02_Drumloop2.wav",
        "samples/trackouts/dnb/03_Kick1.wav",
        "samples/trackouts/dnb/04_Kick2.wav",
        "samples/trackouts/dnb/05_Snare.wav",
        "samples/trackouts/dnb/06_Ride1.wav",
        "samples/trackouts/dnb/07_Ride2.wav",
        "samples/trackouts/dnb/08_HiHat.wav",
        "samples/trackouts/dnb/09_SFX1.wav",
        "samples/trackouts/dnb/10_SFX2.wav",
        "samples/trackouts/dnb/11_Bass1.wav",
        "samples/trackouts/dnb/12_Bass2.wav",
        "samples/trackouts/dnb/13_BassSub.wav",
        "samples/trackouts/dnb/14_Strings1.wav",
        "samples/trackouts/dnb/15_Strings2.wav",
        // "samples/crave.wav",
    ];

    let mut all_clips: Vec<clip::AudioClip> = clip_paths
        .iter()
        .map(|path| {
            clip::AudioClip::new(Path::new(path), Some(Duration::from_millis(10_000)), None)
                .expect("Failed to create audio clip")
        })
        .collect();

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let config = device
        .supported_output_configs()
        .unwrap()
        .next()
        .unwrap()
        .with_sample_rate(SampleRate(b_config.sample_rate))
        .config();

    let rb = b_config.rb;
    let (mut producer, mut consumer) = rb.split();

    // MPC channels for communication
    let (tx_playback, rx_playback) = mpsc::channel::<PlaybackMessage>();
    let (tx_control, rx_control) = mpsc::channel::<ProcessingControlMessage>();
    let tx_control_stream = tx_control.clone();
    let tx_control_worker = tx_control.clone();
    // Prevent buffer underrun at start
    tx_playback.send(PlaybackMessage::RequestData).unwrap();

    // Atomic flag to indicate when audio is done
    let audio_done = Arc::new(AtomicBool::new(false));
    let audio_d_on_stream = audio_done.clone();
    let audio_d_on_worker = audio_done.clone();
    let first_run = Arc::new(AtomicBool::new(false));

    /* Audio output stream */
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                if consumer.occupied_len() < b_config.tolerance {
                    tx_control_stream
                        .send(ProcessingControlMessage::RequestData)
                        .ok();
                    if tx_playback.send(PlaybackMessage::RequestData).is_err() {
                        audio_d_on_stream.store(true, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                }
                for (idx, sample) in data.iter_mut().enumerate() {
                    // Has audio data
                    if consumer.occupied_len() > 0 {
                        let _ = consumer.try_pop().unwrap_or(0.0);
                        let val = consumer.try_pop().unwrap_or(0.0);
                        *sample = val;
                    } else {
                        if !audio_d_on_stream.load(std::sync::atomic::Ordering::Relaxed) {
                            tx_control_stream
                                .send(ProcessingControlMessage::BufferUnderrun)
                                .ok();
                        }
                        *sample = 0.0; // empty audio
                    }
                }
            },
            move |err| {
                print!("An error occured");
            },
            None,
        )
        .unwrap();
    stream.play().unwrap();

    /* Worker thread for decoding and buffering */
    let mut sample_buf = all_clips
        .iter()
        .map(|_| None)
        .collect::<Vec<Option<SampleBuffer<f32>>>>();
    thread::spawn(move || {
        first_run.store(true, std::sync::atomic::Ordering::Relaxed);
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
                            audio_d_on_worker.store(true, std::sync::atomic::Ordering::Relaxed);
                            break 'outer;
                        }
                    }
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });

    /* Main thread for control messages and UI */
    while !audio_done.load(std::sync::atomic::Ordering::Relaxed) {
        match rx_control.recv() {
            Ok(msg) => {
                // Clear terminal
                // print!("\x1B[2J\x1B[1;1H");
                // std::io::stdout().flush().unwrap();
                match msg {
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
                    ProcessingControlMessage::BufferRecharge => {
                        // println!("🔄 Recharging buffer");
                    }
                }
            }
            Err(_) => break,
        }
    }

    while !audio_done.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
