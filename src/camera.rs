use opencv::core::{Mat, Point3_, Size, ToInputArray};
use opencv::{imgproc, prelude::*, videoio};

const ASCII_CHAR_H_OVER_W: f64 = 2.5;

pub struct Camera {
    cam: videoio::VideoCapture,
    frame: Mat,
}

impl Camera {
    // Constructs a new Camera object
    pub fn new(cam_width: f64, cam_height: f64, cam_fps: f64) -> Option<Camera> {
        let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY).unwrap();
        let frame = Mat::default();

        cam.set(videoio::CAP_PROP_FRAME_WIDTH, cam_width).unwrap();
        cam.set(videoio::CAP_PROP_FRAME_HEIGHT, cam_height).unwrap();
        cam.set(videoio::CAP_PROP_FPS, cam_fps).unwrap();

        let opened = videoio::VideoCapture::is_opened(&cam).unwrap();

        if !opened {
            eprintln!("Unable to open default camera!");
            return None;
        }

        Some(Camera { cam, frame })
    }

    // Resizes a Mat to the specified width and height
    pub fn resize_frame(&mut self, new_width: f64, new_height: f64, preserve_aspect_ratio: bool) {
        let (orig_width, orig_height) = (
            self.frame.cols() as f64 * ASCII_CHAR_H_OVER_W,
            self.frame.rows() as f64,
        );
        let orig_ratio = orig_width / orig_height;

        let new_size = match preserve_aspect_ratio {
            true => {
                let new_ratio = new_width / new_height;
                if new_ratio > orig_ratio {
                    Size {
                        width: (new_height * orig_ratio) as i32,
                        height: new_height as i32,
                    }
                } else {
                    Size {
                        width: new_width as i32,
                        height: (new_width / orig_ratio) as i32,
                    }
                }
            }
            false => Size {
                width: new_width as i32,
                height: new_height as i32,
            },
        };

        imgproc::resize(
            &self.frame.input_array().unwrap(),
            &mut self.frame,
            new_size,
            0.0,
            0.0,
            opencv::imgproc::INTER_LINEAR,
        )
        .unwrap();
    }

    // changes color depth by rounding each color channel to the nearest multiple of 255/new_colors_per_channel
    pub fn change_color_depth(&mut self, colors_per_channel: u8) {
        let data = self.frame.data_typed_mut::<Point3_<u8>>().unwrap();

        let multiple = 255 / colors_per_channel;

        // rounds each r, g, b to nearest multiple of 255/new_colors_per_channel and clamps to 0-255
        let convert = |rgb_value: u8| {
            ((rgb_value as f64 / multiple as f64).round() * multiple as f64).clamp(0.0, 255.0) as u8
        };

        // convert each pixel
        for pixel in &mut *data {
            pixel.x = convert(pixel.x);
            pixel.y = convert(pixel.y);
            pixel.z = convert(pixel.z);
        }

        // set data back to frame
        let data_ptr: *const u8 = data.as_ptr() as *const u8;
        unsafe { self.frame.set_data(data_ptr) }
    }

    pub fn get_frame(&self) -> &Mat {
        &self.frame
    }

    pub fn read_frame(&mut self) -> bool {
        self.cam.read(&mut self.frame).unwrap()
    }

    pub fn get_width(&self) -> f64 {
        self.cam.get(videoio::CAP_PROP_FRAME_WIDTH).unwrap()
    }

    pub fn get_height(&self) -> f64 {
        self.cam.get(videoio::CAP_PROP_FRAME_HEIGHT).unwrap()
    }

    pub fn get_fps(&self) -> f64 {
        self.cam.get(videoio::CAP_PROP_FPS).unwrap()
    }

    pub fn get_property(&self, prop: i32) -> f64 {
        self.cam.get(prop).unwrap()
    }
}
