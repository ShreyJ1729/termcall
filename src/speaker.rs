use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

struct Speaker {
    device: cpal::Device,
    stream: cpal::Stream,
}

impl Speaker {
    fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("Failed to get default output device");

        let config = device
            .default_output_config()
            .expect("Failed to get default output config");

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &_| {
                    // here, write to the mutuable slice `data` to play audio
                    // call some function to get latest audio data from webrtc
                },
                move |err| {
                    // handle errors
                },
                None,
            )
            .expect("Failed to build output stream");

        Self { device, stream }
    }

    fn play(&self) {
        self.stream.play().expect("Failed to play stream");
    }

    fn pause(&self) {
        self.stream.pause().expect("Failed to pause stream");
    }
}
