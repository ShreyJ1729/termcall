use anyhow::Result;
use opencv::core::Mat;
use opencv::videoio::VideoCapture;
use opencv::{prelude::*, videoio};

pub struct Camera {
    cam: videoio::VideoCapture,
}

impl Camera {
    pub fn new() -> Camera {
        let cam: VideoCapture = VideoCapture::default().unwrap();
        Camera { cam }
    }

    pub fn init(
        &mut self,
        cam_width: f64,
        cam_height: f64,
        cam_fps: f64,
        cam_index: i32,
    ) -> Result<()> {
        self.cam = videoio::VideoCapture::new(cam_index, videoio::CAP_ANY)?;
        self.cam.set(videoio::CAP_PROP_FRAME_WIDTH, cam_width)?;
        self.cam.set(videoio::CAP_PROP_FRAME_HEIGHT, cam_height)?;
        self.cam.set(videoio::CAP_PROP_FPS, cam_fps)?;

        match videoio::VideoCapture::is_opened(&self.cam) {
            Ok(true) => Ok(()),
            Ok(false) => Err(anyhow::anyhow!("Camera is not opened")),
            Err(e) => Err(anyhow::anyhow!("Error opening camera: {}", e)),
        }
    }

    // For efficiency, read camera data directly into Frame object Mat
    pub fn read_frame(&mut self, frame_ref: &mut Mat) -> Result<()> {
        self.cam.read(frame_ref)?;
        Ok(())
    }
}
