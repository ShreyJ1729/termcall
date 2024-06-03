use anyhow::Result;
use image::{ImageBuffer, Rgb};
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{
        CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
    },
};

pub const CAMERA_WIDTH: u32 = 640;
pub const CAMERA_HEIGHT: u32 = 480;
pub const CAMERA_FPS: u32 = 30;

pub struct Camera {
    camera: nokhwa::Camera,
}

impl Camera {
    pub fn new() -> Camera {
        let index = CameraIndex::Index(0);
        let resolution = Resolution::new(CAMERA_WIDTH as u32, CAMERA_HEIGHT as u32);
        let camera_format = CameraFormat::new(resolution, FrameFormat::RAWRGB, CAMERA_FPS);

        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(camera_format));
        let camera = nokhwa::Camera::new(index, requested).unwrap();

        Self { camera }
    }

    pub fn read_frame(&mut self, frame_ref: &mut ImageBuffer<Rgb<u8>, Vec<u8>>) -> Result<()> {
        let frame = self.camera.frame()?;
        let decoded = frame.decode_image::<RgbFormat>()?;
        *frame_ref = decoded;

        Ok(())
    }
}
