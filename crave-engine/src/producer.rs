use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Observer, Producer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;

use crate::{
    audio_config::AudioBufferConfig,
    clip::AudioClip,
    messages::{ProducerCommand, ProducerStatus},
};

type RingBufferProducer = ringbuf::CachingProd<Arc<SharedRb<Heap<f32>>>>;

pub struct AudioProducer {
    clips: Vec<AudioClip>,
    producer: RingBufferProducer,
    config: AudioBufferConfig,
    sample_buffers: Vec<Option<SampleBuffer<f32>>>,
}

impl AudioProducer {
    pub fn new(
        clips: Vec<AudioClip>,
        producer: RingBufferProducer,
        config: AudioBufferConfig,
    ) -> Self {
        let sample_buffers = clips.iter().map(|_| None).collect();

        Self {
            clips,
            producer,
            config,
            sample_buffers,
        }
    }

    pub fn start_production(
        mut self,
        rx_playback: Receiver<ProducerCommand>,
        tx_control: Sender<ProducerStatus>,
        audio_done: Arc<AtomicBool>,
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
                                        audio_done.store(true, Ordering::Relaxed);
                                        break 'outer;
                                    }
                                }
                            }
                            Err(_) => {
                                // Error occurred, signal completion
                                tx_control.send(ProducerStatus::DecodingDone).ok();
                                audio_done.store(true, Ordering::Relaxed);
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
        let mut any_decoded = false;

        // Decode from all clips and mix them together
        for (idx, clip) in self.clips.iter_mut().enumerate() {
            match clip.decode() {
                Ok(decoded) => {
                    any_decoded = true;
                    if self.sample_buffers[idx].is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        self.sample_buffers[idx] = Some(SampleBuffer::<f32>::new(duration, spec));
                    }

                    if let Some(buf) = &mut self.sample_buffers[idx] {
                        buf.copy_interleaved_ref(decoded);
                        let samples = buf.samples();

                        // Initialize or mix samples
                        if mixed_samples.is_empty() {
                            mixed_samples = samples.to_vec();
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
                }
                Err(Error::ResetRequired) => {
                    println!("Decoder reset required for a clip");
                    return Err(Error::ResetRequired);
                }
                Err(_) => {
                    // This clip is done, continue with other clips
                }
            }
        }

        if any_decoded {
            Ok(mixed_samples)
        } else {
            Ok(Vec::new()) // All clips are done
        }
    }
}
