#![allow(unused_variables)]
use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;
use wav_decoder::WaveDecoder;

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

// Buffer constants
const B_SAMPLE_RATE: u32 = 44100;
const B_LOOKAHEAD: usize = 10; // Lookahead in seconds
const B_CAPACITY: usize = ((B_SAMPLE_RATE) as usize * 2) * B_LOOKAHEAD;
const B_THRESHOLD: usize = 1024; // Threshold to consider buffer "full"
const B_TOLERANCE: usize = B_CAPACITY / 2;

fn main() {
    // Initialize decoder
    let file_path = Path::new("samples/crave.wav");
    let file = std::fs::File::open(file_path).expect("Failed to open file");
    let mut decoder = WaveDecoder::try_new(file).expect("Failed to create decoder");
    let mut sample_buf = None;

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let config = device
        .supported_output_configs()
        .unwrap()
        .next()
        .unwrap()
        .with_sample_rate(SampleRate(B_SAMPLE_RATE))
        .config();

    // Ring buffer for audio samples
    let rb = SharedRb::<Heap<f32>>::new(B_CAPACITY);
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

    /* Audio output stream */
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                if consumer.occupied_len() < B_TOLERANCE {
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
    thread::spawn(move || {
        'outer: loop {
            if let Ok(msg) = rx_playback.try_recv() {
                loop {
                    if producer.occupied_len() > B_CAPACITY - B_THRESHOLD {
                        tx_control_worker
                            .send(ProcessingControlMessage::BufferFull)
                            .ok();
                        break;
                    }
                    match decoder.decode() {
                        Ok(_decoded) => {
                            if sample_buf.is_none() {
                                let spec = *_decoded.spec();
                                let duration = _decoded.capacity() as u64;

                                sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                            }
                            if let Some(buf) = &mut sample_buf {
                                tx_control_worker
                                    .send(ProcessingControlMessage::BufferRecharge)
                                    .ok();
                                buf.copy_interleaved_ref(_decoded);
                                producer.push_slice(buf.samples());
                            }
                        }
                        Err(Error::ResetRequired) => break,
                        Err(err) => {
                            tx_control_worker
                                .send(ProcessingControlMessage::DecodingDone)
                                .ok();
                            if producer.is_empty() {
                                println!("Audio done");
                                stream.pause().expect("Failed to pause stream");
                                audio_d_on_worker.store(true, std::sync::atomic::Ordering::Relaxed);
                                break 'outer;
                            }
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
