use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Observer, Producer};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use symphonia::core::errors::Error;

use crate::decoder::Decoder;
use crate::{
    audio_config::AudioBufferConfig,
    clip::AudioClip,
    messages::{ProducerCommand, ProducerStatus},
};

type RingBufferProducer = ringbuf::CachingProd<Arc<SharedRb<Heap<f32>>>>;

pub struct AudioProducer<T>
where
    T: Decoder,
{
    clips: Vec<AudioClip<T>>,
    producer: RingBufferProducer,
    config: AudioBufferConfig,
}

impl<T> AudioProducer<T>
where
    T: Decoder + Send + 'static,
{
    pub fn new(
        clips: Vec<AudioClip<T>>,
        producer: RingBufferProducer,
        config: AudioBufferConfig,
    ) -> Self {
        Self {
            clips,
            producer,
            config,
        }
    }

    pub fn start_production(
        mut self,
        rx_playback: Receiver<ProducerCommand>,
        tx_control: Sender<ProducerStatus>,
    ) {
        thread::spawn(move || {
            'outer: loop {
                if let Ok(_msg) = rx_playback.try_recv() {
                    loop {
                        if self.producer.occupied_len() > self.config.threshold {
                            tx_control.send(ProducerStatus::BufferFull).ok();
                            break;
                        }

                        match self.decode_and_mix() {
                            Ok(mixed_samples) => {
                                if !mixed_samples.is_empty() {
                                    tx_control.send(ProducerStatus::BufferRecharge).ok();
                                    self.producer.push_slice(&mixed_samples);
                                } else {
                                    // All clips are done
                                    tx_control.send(ProducerStatus::DecodingDone).ok();
                                    if self.producer.is_empty() {
                                        break 'outer;
                                    }
                                }
                            }
                            Err(_) => {
                                // Error occurred, signal completion
                                tx_control.send(ProducerStatus::DecodingDone).ok();
                                break 'outer;
                            }
                        }
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        });
    }

    fn decode_and_mix(&mut self) -> Result<Vec<f32>, Error> {
        let mut mixed_samples: Vec<f32> = Vec::new();

        // Decode from all clips and mix them together
        for (idx, clip) in self.clips.iter_mut().enumerate() {
            let samples = clip.decode().unwrap();

            // Initialize or mix samples
            if mixed_samples.is_empty() {
                mixed_samples = samples
            } else {
                // Mixing multiple clips
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

        Ok(mixed_samples)
    }
}
