use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait};
use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::{AudioProducerMessage, buffer_config};

type RingBuffer = ringbuf::CachingCons<Arc<SharedRb<Heap<f32>>>>;

pub struct AudioPlayer {
    consumer: Arc<Mutex<RingBuffer>>,
    data_request_tx: Sender<AudioProducerMessage>,
    config: buffer_config::AudioBufferConfig,
}

impl AudioPlayer {
    pub fn new(
        consumer: RingBuffer,
        data_request_tx: Sender<AudioProducerMessage>,
        config: buffer_config::AudioBufferConfig,
    ) -> Self {
        Self {
            consumer: Arc::new(Mutex::new(consumer)),
            data_request_tx,
            config,
        }
    }

    pub fn create_stream(&self) -> cpal::Stream {
        let device = self.get_audio_device();
        let stream_config = self.get_stream_config(&device);

        let consumer = self.consumer.clone();
        let data_request_tx = self.data_request_tx.clone();
        let tolerance = self.config.tolerance;

        device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut consumer = consumer.lock().unwrap();

                    if consumer.occupied_len() < tolerance {
                        let _ = data_request_tx.send(AudioProducerMessage::RequestData);
                    }

                    for sample in data.iter_mut() {
                        if consumer.occupied_len() > 0 {
                            let _ = consumer.try_pop().unwrap_or(0.0); // Skip left channel or first sample
                            *sample = consumer.try_pop().unwrap_or(0.0); // Get right channel or actual sample
                        } else {
                            *sample = 0.0;
                        }
                    }
                },
                |err| eprintln!("Audio stream error: {:?}", err),
                None,
            )
            .expect("Failed to create audio stream")
    }

    fn get_audio_device(&self) -> cpal::Device {
        cpal::default_host()
            .default_output_device()
            .expect("No output device available")
    }

    fn get_stream_config(&self, device: &cpal::Device) -> cpal::StreamConfig {
        device
            .supported_output_configs()
            .expect("No supported output configs")
            .next()
            .expect("No output config available")
            .with_sample_rate(SampleRate(self.config.sample_rate))
            .config()
    }
}
