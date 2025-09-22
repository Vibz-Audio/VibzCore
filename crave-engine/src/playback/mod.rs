use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel::{Receiver, Sender, unbounded};
use std::sync::Arc;

use crate::buffer_config;

pub enum PlaybackCommand {
    Play,
    Pause,
    Set, // TODO: add timeline position
}

enum PlaybackState {
    Playing,
    Paused,
}

pub struct PlaybackController {
    pub state: Arc<PlaybackState>,
    pub sender: Sender<PlaybackState>,
    pub recv: Receiver<PlaybackState>,
}

impl PlaybackController {
    pub fn new() -> Self {
        let (sender, recv) = unbounded();
        Self {
            state: PlaybackState::Paused.into(),
            sender,
            recv,
        }
    }

    pub fn dispatch(&mut self, command: PlaybackCommand) {
        match command {
            PlaybackCommand::Play => self.play(),
            PlaybackCommand::Pause => self.pause(),
            PlaybackCommand::Set => self.set_position(),
        }
    }

    pub fn stream(&self) {
        let PlaybackConfig { device, config, .. } = PlaybackConfig::new();

        let pb_state = self.recv.clone();

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], cb: &cpal::OutputCallbackInfo| {
                    println!("OutputCallbackInfo: {:?}", cb);
                    match pb_state.try_recv().unwrap() {
                        PlaybackState::Playing => {
                            println!("PlaybackState::Playing");
                            for sample in data.iter_mut() {
                                *sample = 0.0; // Placeholder for actual audio data
                            }
                        }
                        PlaybackState::Paused => {
                            println!("PlaybackState::Paused");
                            for sample in data.iter_mut() {
                                *sample = 0.0; // Silence when paused
                            }
                        }
                    }
                },
                |err| {
                    eprint!("An error occured");
                },
                None,
            )
            .unwrap();

        stream.play().unwrap();
    }

    fn play(&mut self) {
        if let PlaybackState::Paused = self.state.as_ref() {
            self.state = PlaybackState::Playing.into();
            println!("Playback started");
        }
    }

    fn pause(&mut self) {
        if let PlaybackState::Playing = self.state.as_ref() {
            self.state = PlaybackState::Paused.into();
            println!("Playback paused");
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
