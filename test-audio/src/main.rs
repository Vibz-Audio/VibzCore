#![allow(unused_variables)]
use std::path::Path;
use std::sync::{Arc, Mutex};

use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;
use wav_decoder::WaveDecoder;

fn main() {
    let file_path = Path::new("samples/crave.wav");
    let file = std::fs::File::open(file_path).expect("Failed to open file");

    let mut decoder = WaveDecoder::try_new(file).expect("Failed to create decoder");

    let mut sample_buf = None;
    let mut decorded_buffer: Vec<f32> = vec![];

    loop {
        match decoder.decode() {
            Ok(_decoded) => {
                if sample_buf.is_none() {
                    let spec = *_decoded.spec();

                    let duration = _decoded.capacity() as u64;

                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }
                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(_decoded);
                    decorded_buffer.extend_from_slice(buf.samples());
                }
            }
            Err(Error::ResetRequired) => unimplemented!(),
            Err(err) => break,
        }
    }

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

    let playback_pos = Arc::new(Mutex::new(0));
    let audio_buf = Arc::new(decorded_buffer);

    let audio_buf_clone = Arc::clone(&audio_buf);
    let playback_pos_clone = Arc::clone(&playback_pos);

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                for (idx, sample) in data.iter_mut().enumerate() {
                    let mut pos = playback_pos_clone.lock().unwrap();
                    if *pos < audio_buf_clone.len() {
                        *sample = audio_buf_clone[*pos];
                        *pos += 2;

                        for _ in 0..(*sample * 1000.0) as usize {
                            print!("*");
                        }
                        println!();
                    } else {
                        *sample = 0.0; // empty audio
                    }
                }
            },
            move |err| {
                print!("An error occured");
            },
            None, // None=blocking, Some(Duration)=timeout
        )
        .unwrap();

    stream.play().unwrap();

    std::thread::sleep(std::time::Duration::from_secs(120));
}
