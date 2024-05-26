use opencv::core::Mat;
use opencv::videoio::VideoCapture;
use opencv::{imgcodecs, prelude::*, videoio};

const ASCII_CHAR_H_OVER_W: f64 = 2.25;

pub struct Camera {
    cam: videoio::VideoCapture,
    frame: Mat,
}

impl Camera {
    // Purposefully lightweight to allow for multiple struct instances
    pub fn new() -> Camera {
        let cam: VideoCapture = VideoCapture::default().unwrap();
        let frame = Mat::default();
        Camera { cam, frame }
    }

    pub fn init(&mut self, cam_width: f64, cam_height: f64, cam_fps: f64, cam_index: i32) {
        self.cam = videoio::VideoCapture::new(cam_index, videoio::CAP_ANY).unwrap();
        self.cam
            .set(videoio::CAP_PROP_FRAME_WIDTH, cam_width)
            .unwrap();
        self.cam
            .set(videoio::CAP_PROP_FRAME_HEIGHT, cam_height)
            .unwrap();
        self.cam.set(videoio::CAP_PROP_FPS, cam_fps).unwrap();

        if !videoio::VideoCapture::is_opened(&self.cam).unwrap() {
            eprintln!("Unable to open default camera!");
        }
    }

    pub fn get_frame(&self) -> &Mat {
        &self.frame
    }

    pub fn mat_to_bytes(&self) -> Vec<u8> {
        let mut buf = opencv::types::VectorOfu8::new();
        imgcodecs::imencode(".jpg", &self.frame, &mut buf, &opencv::core::Vector::new()).unwrap();
        buf.to_vec()
    }

    pub fn read_frame(&mut self) -> bool {
        self.cam.read(&mut self.frame).unwrap()
    }
}
