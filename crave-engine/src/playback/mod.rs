use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel::{Receiver, Sender as SenderCB, unbounded};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer, ring_buffer};
use ringbuf::{HeapRb, SharedRb};
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
    pub buffer_tx: PlaybackMsg,
}

impl PlaybackController {
    pub fn new(buffer: RingBuffer, pb_msg: PlaybackMsg) -> Self {
        let (sender, recv) = unbounded();
        Self {
            state: PlaybackState::Paused.into(),
            sender,
            recv,
            buffer_rx: Arc::new(buffer.into()),
            buffer_tx: pb_msg,
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

        device
            .build_output_stream(
                &config,
                move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                    let mut consumer = consumer.lock().unwrap();

                    if consumer.occupied_len() < data.len() {
                        // If there's not enough data, send a message to pause playback
                        let _ = pb_state.try_recv();
                    }

                    for sample in data.iter_mut() {
                        let _ = consumer.try_pop().unwrap_or(0.0);
                        *sample = consumer.try_pop().unwrap_or(0.0);
                    }
                    drop(consumer);
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
