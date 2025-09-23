use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait};
use crossbeam::channel::{Receiver, Sender as CbSender, unbounded};
use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::{messages::{ProducerCommand, PlayerCommand}, audio_config};

const SILENCE_SAMPLE: f32 = 0.0;

type RingBuffer = ringbuf::CachingCons<Arc<SharedRb<Heap<f32>>>>;

pub struct AudioPlayer {
    consumer: Arc<Mutex<RingBuffer>>,
    data_request_tx: Sender<ProducerCommand>,
    config: audio_config::AudioBufferConfig,
    audio_player_cmd_rx: Receiver<PlayerCommand>,
    is_paused: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new(
        consumer: RingBuffer,
        data_request_tx: Sender<ProducerCommand>,
        config: audio_config::AudioBufferConfig,
    ) -> (Self, CbSender<PlayerCommand>) {
        let (command_tx, command_rx) = unbounded();
        let player = Self {
            consumer: Arc::new(Mutex::new(consumer)),
            data_request_tx,
            config,
            audio_player_cmd_rx: command_rx,
            is_paused: Arc::new(AtomicBool::new(false)), // Start playing
        };
        (player, command_tx)
    }

    pub fn dispatch(&self, command: PlayerCommand) {
        match command {
            PlayerCommand::Play => self.is_paused.store(false, Ordering::Relaxed),
            PlayerCommand::Pause => self.is_paused.store(true, Ordering::Relaxed),
            PlayerCommand::TogglePlayPause => {
                let was_paused = self.is_paused.load(Ordering::Relaxed);
                self.is_paused.store(!was_paused, Ordering::Relaxed);
            }
        }
    }

    pub fn process_commands(&self) {
        while let Ok(command) = self.audio_player_cmd_rx.try_recv() {
            self.dispatch(command);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed)
    }

    pub fn create_stream(&self) -> cpal::Stream {
        let device = self.get_audio_device();
        let stream_config = self.get_stream_config(&device);

        let consumer = self.consumer.clone();
        let data_request_tx = self.data_request_tx.clone();
        let tolerance = self.config.tolerance;
        let is_paused = self.is_paused.clone();

        device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if is_paused.load(Ordering::Relaxed) {
                        for sample in data.iter_mut() {
                            *sample = SILENCE_SAMPLE
                        }
                        return;
                    }

                    let mut consumer = consumer.lock().unwrap();

                    if consumer.occupied_len() < tolerance {
                        let _ = data_request_tx.send(ProducerCommand::RequestData);
                    }

                    for sample in data.iter_mut() {
                        if consumer.occupied_len() > 0 {
                            let _ = consumer.try_pop().unwrap_or(SILENCE_SAMPLE); // Skip left channel or first sample
                            *sample = consumer.try_pop().unwrap_or(SILENCE_SAMPLE); // Get right channel or actual sample
                        } else {
                            *sample = SILENCE_SAMPLE;
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
