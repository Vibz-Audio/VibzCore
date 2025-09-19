#![allow(unused_variables)]
use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;

use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;
use wav_decoder::WaveDecoder;

fn main() {
    let file_path = Path::new("samples/crave.wav");
    let file = std::fs::File::open(file_path).expect("Failed to open file");

    let mut decoder = WaveDecoder::try_new(file).expect("Failed to create decoder");
    let mut sample_buf = None;

    let lookahead = 10;
    let capacity = (44100 * 2) * lookahead;
    let tolerance = capacity / 2;
    let rb = SharedRb::<Heap<f32>>::new(capacity); // 1 second buffer for
    // stereo f32 audio
    let (mut producer, mut consumer) = rb.split();

    enum AudioMessage {
        NeedData,
    }
    let (tx, rx) = mpsc::channel::<AudioMessage>();

    enum ProcessingControlMessage {
        RequestData,
        DecodingDone,
        BufferFull,
        BufferRecharge,
        BufferUnderrun,
    }
    let (tx_control, rx_control) = mpsc::channel::<ProcessingControlMessage>();

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let config = device
        .supported_output_configs()
        .unwrap()
        .next()
        .unwrap()
        .with_sample_rate(SampleRate(44100))
        .config();

    let tx_clone = tx.clone();

    let audio_done = Arc::new(AtomicBool::new(false));
    let audio_d_stream = audio_done.clone();
    let audio_d_worker = audio_done.clone();

    // Prevent buffer underrun at start
    tx.send(AudioMessage::NeedData).unwrap();

    let tx_control_stream = tx_control.clone();
    let tx_control_worker = tx_control.clone();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                if consumer.occupied_len() < tolerance {
                    tx_control_stream
                        .send(ProcessingControlMessage::RequestData)
                        .ok();
                    if tx.send(AudioMessage::NeedData).is_err() {
                        // worker thread has stopped
                        // We can assume audio is done
                        audio_d_stream.store(true, std::sync::atomic::Ordering::Relaxed);
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
                        if !audio_d_stream.load(std::sync::atomic::Ordering::Relaxed) {
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

    thread::spawn(move || {
        'outer: loop {
            match rx.try_recv() {
                Ok(msg) => match msg {
                    AudioMessage::NeedData => loop {
                        if producer.occupied_len() > capacity - 1024 {
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
                                    audio_d_worker
                                        .store(true, std::sync::atomic::Ordering::Relaxed);
                                    break 'outer;
                                }
                            }
                        }
                    },
                },
                Err(_) => {
                    // No message received, can perform other tasks or just sleep
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
    });

    // UI display
    loop {
        if let Ok(msg) = rx_control.recv() {
            // Clear the terminal screen
            print!("\x1B[2J\x1B[1;1H");
            std::io::stdout().flush().unwrap();

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
                    println!("🔄 Recharging buffer");
                }
            }
        } else {
            break;
        }
    }

    while !audio_done.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
