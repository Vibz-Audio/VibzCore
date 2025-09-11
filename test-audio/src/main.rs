#![allow(unused_variables)]
use std::path::Path;
use std::sync::{Arc, Mutex};

use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

fn main() {
    let file_path = Path::new("samples/audio.wav");

    let src = std::fs::File::open(file_path).expect("failed to open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("wav");

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .expect("unsupported format");

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    let dec_opts: DecoderOptions = Default::default();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .expect("unsupported codec");

    let track_id = track.id;

    let mut sample_buf = None;
    let mut decorded_buffer: Vec<f32> = vec![];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => unimplemented!(),
            Err(err) => break,
        };
        match decoder.decode(&packet) {
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
            Err(err) => {
                panic!("{}", err);
            }
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
        .with_sample_rate(SampleRate(48000))
        .config();

    let playback_pos = Arc::new(Mutex::new(10000));
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
                        *pos += 1;
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

    std::thread::sleep(std::time::Duration::from_secs(12));
}
