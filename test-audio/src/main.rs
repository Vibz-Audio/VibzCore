#![allow(unused_variables)]
use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::Path;
use std::thread;
use std::time::Instant;

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

    let capacity = (44100 * 2) * 2;
    let rb = SharedRb::<Heap<f32>>::new(capacity); // 1 second buffer for
    // stereo f32 audio
    let (mut producer, mut consumer) = rb.split();

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

    thread::spawn(move || {
        println!("Starting decoder thread");
        loop {
            loop {
                println!(
                    "Producer len: {} / capacity: {}",
                    producer.occupied_len(),
                    capacity
                );
                if producer.occupied_len() >= (capacity / 2) {
                    println!("Producer full, sleeping...");
                    break;
                }

                // println!("Decoding audio...");
                match decoder.decode() {
                    Ok(_decoded) => {
                        if sample_buf.is_none() {
                            let spec = *_decoded.spec();
                            let duration = _decoded.capacity() as u64;

                            sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                        }
                        if let Some(buf) = &mut sample_buf {
                            buf.copy_interleaved_ref(_decoded);
                            producer.push_slice(buf.samples());
                        }
                    }
                    Err(Error::ResetRequired) => {
                        println!("Decoder reset required");
                        break;
                    }
                    Err(err) => {
                        eprintln!("Error decoding audio: {:?}", err);
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    thread::spawn(move || {
        println!("Starting playback thread");
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                    for (idx, sample) in data.iter_mut().enumerate() {
                        // println!("Streaming");
                        if consumer.occupied_len() > 0 {
                            let _ = consumer.try_pop().unwrap_or(0.0);
                            let val = consumer.try_pop().unwrap_or(0.0);
                            // println!("Sample[{}] = {}", idx, val);
                            *sample = val;
                        } else {
                            println!("Buffer underrun!");
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

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(120));
}
