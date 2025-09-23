use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait};
use crossbeam::channel::{Receiver, Sender as SenderCB, unbounded};
use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::{PlaybackMessage, buffer_config};

pub enum PlaybackCommand {
    Play,
    Pause,
    Set, // TODO: add timeline position
}

enum PlaybackState {
    Playing,
    Paused,
}

type RingBuffer = ringbuf::CachingCons<Arc<SharedRb<Heap<f32>>>>;
type PlaybackMsg = Sender<PlaybackMessage>;

pub struct PlaybackController {
    pub state: Arc<PlaybackState>,
    pub sender: SenderCB<PlaybackState>,
    pub recv: Receiver<PlaybackState>,
    pub buffer_rx: Arc<Mutex<RingBuffer>>,
    pub producer_tx: PlaybackMsg,
    pub buffer_config: buffer_config::AudioBufferConfig,
}

impl PlaybackController {
    pub fn new(
        buffer: RingBuffer,
        pb_msg: PlaybackMsg,
        buffer_config: buffer_config::AudioBufferConfig,
    ) -> Self {
        let (sender, recv) = unbounded();
        Self {
            state: PlaybackState::Paused.into(),
            sender,
            recv,
            buffer_rx: Arc::new(buffer.into()),
            producer_tx: pb_msg,
            buffer_config,
        }
    }

    pub fn dispatch(&mut self, command: PlaybackCommand) {
        match command {
            PlaybackCommand::Play => self.play(),
            PlaybackCommand::Pause => self.pause(),
            PlaybackCommand::Set => self.set_position(),
        }
    }

    pub fn stream(&self) -> cpal::Stream {
        let PlaybackConfig { device, config, .. } = PlaybackConfig::new();
        let pb_state = self.recv.clone();
        let consumer = self.buffer_rx.clone();
        let producer_tx = self.producer_tx.clone();
        let buffer_config = self.buffer_config.clone();

        device
            .build_output_stream(
                &config,
                move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                    if let Ok(PlaybackState::Paused) = pb_state.try_recv() {
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                        return;
                    }

                    let mut consumer = consumer.lock().unwrap();

                    if consumer.occupied_len() < buffer_config.tolerance {
                        if producer_tx.send(PlaybackMessage::RequestData).is_err() {
                            for sample in data.iter_mut() {
                                *sample = 0.0;
                            }
                            return;
                        }
                    }

                    for sample in data.iter_mut() {
                        if consumer.occupied_len() > 0 {
                            let _ = consumer.try_pop().unwrap_or(0.0);
                            *sample = consumer.try_pop().unwrap_or(0.0);
                        } else {
                            *sample = 0.0; // empty audio when buffer is empty
                        }
                    }
                },
                |err| {
                    eprint!("An error occured");
                },
                None,
            )
            .unwrap()
    }

    fn play(&mut self) {
        if let PlaybackState::Paused = self.state.as_ref() {
            self.state = PlaybackState::Playing.into();
        }
    }

    fn pause(&mut self) {
        if let PlaybackState::Playing = self.state.as_ref() {
            self.state = PlaybackState::Paused.into();
            self.sender.send(PlaybackState::Paused).unwrap();
        }
    }

    fn set_position(&self) {
        // Placeholder for setting timeline position
        println!("Set position command received (not implemented)");
    }
}
pub struct PlaybackConfig {
    pub host: cpal::Host,
    pub device: cpal::Device,
    pub config: cpal::StreamConfig,
}

impl PlaybackConfig {
    // TODO: handle errors properly, change name to try_new
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");

        let buffer_config = buffer_config::AudioBufferConfig::default();

        let config = device
            .supported_output_configs()
            .unwrap()
            .next()
            .unwrap()
            .with_sample_rate(SampleRate(buffer_config.sample_rate))
            .config();

        Self {
            host,
            device,
            config,
        }
    }
}
