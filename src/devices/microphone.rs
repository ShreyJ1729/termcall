use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use simple_log::{error, info};
use webrtc::data_channel::RTCDataChannel;

pub struct Microphone {
    stream: cpal::Stream,
}

impl Microphone {
    pub fn new(audio_send_dc: Arc<RTCDataChannel>) -> Self {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("Failed to get default input device");

        let config = device
            .default_input_config()
            .expect("Failed to get default input config");

        let audio_send_dc = audio_send_dc.clone();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1000);

        tokio::spawn(async move {
            while let Some(payload) = rx.recv().await {
                match audio_send_dc.send(&payload).await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("error sending audio stream: {}", err);
                    }
                }
                info!(
                    "sent {} bytes over {}",
                    payload.len().to_string(),
                    audio_send_dc.label()
                );
            }
        });

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    let data = bincode::serialize(data).unwrap();
                    let payload = bytes::Bytes::from(data);
                    match tx.try_send(payload) {
                        Ok(_) => {}
                        Err(err) => {
                            error!("error sending audio stream: {}", err);
                        }
                    }
                },
                move |err| {
                    error!("error sending audio stream: {}", err);
                },
                None,
            )
            .expect("Failed to build input stream");

        stream.play().expect("Failed to play stream");

        Self { stream }
    }

    pub fn listen(&self) {
        self.stream.play().expect("Failed to play stream");
    }

    pub fn mute(&self) {
        self.stream.pause().expect("Failed to pause stream");
    }
}
