use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

struct Microphone {
    device: cpal::Device,
    stream: cpal::Stream,
}

impl Microphone {
    fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("Failed to get default input device");

        let config = device
            .default_input_config()
            .expect("Failed to get default input config");

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    // send mic data over webrtc
                },
                move |err| {
                    // handle errors
                },
                None,
            )
            .expect("Failed to build input stream");

        Self { device, stream }
    }

    fn listen(&self) {
        self.stream.play().expect("Failed to play stream");
    }

    fn mute(&self) {
        self.stream.pause().expect("Failed to pause stream");
    }
}
