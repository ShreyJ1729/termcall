use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use simple_log::error;
use tokio::sync::Mutex;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

pub struct Speaker {
    device: cpal::Device,
    stream: cpal::Stream,
}

impl Speaker {
    pub fn new(mut rx: tokio::sync::mpsc::Receiver<DataChannelMessage>) -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("Failed to get default output device");

        let config = device
            .default_output_config()
            .expect("Failed to get default output config");

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &_| {
                    // here, write to the mutuable slice `data` to play audio
                    match rx.try_recv() {
                        Ok(payload) => {
                            let payload: Vec<f32> = bincode::deserialize(&payload.data).unwrap();
                            for (i, sample) in payload.iter().enumerate() {
                                data[i] = *sample;
                            }
                        }
                        Err(_) => {
                            for sample in data.iter_mut() {
                                *sample = 0.0;
                            }
                            error!("no audio data received");
                        }
                    }
                },
                move |err| {
                    error!("error receiving audio stream: {}", err);
                },
                None,
            )
            .expect("Failed to build output stream");

        stream.play().expect("Failed to play stream");

        Self { device, stream }
    }

    pub fn play(&self) {
        self.stream.play().expect("Failed to play stream");
    }

    pub fn pause(&self) {
        self.stream.pause().expect("Failed to pause stream");
    }
}
