use opencv::core::{Mat, Point3_};
use opencv::prelude::*;
use std::io::{self, Write};
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

pub struct Terminal {
    pub stdout: io::Stdout,
    pub width: i32,
    pub height: i32,
}

impl Terminal {
    pub fn new() -> Terminal {
        let stdout = io::stdout();
        let (width, height) = termion::terminal_size().unwrap();
        Terminal {
            stdout,
            width: width as i32,
            height: height as i32,
        }
    }

    pub fn goto_topleft(&mut self) {
        write!(self.stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();
    }

    pub fn write_to_bottomright(&mut self, s: &str) {
        write!(
            self.stdout,
            "{}{}",
            termion::cursor::Goto(self.width as u16 - s.len() as u16, self.height as u16),
            s
        )
        .unwrap();
    }

    pub fn clear(&mut self) {
        write!(self.stdout, "{}", termion::clear::All).unwrap();
    }

    pub fn hide_cursor(&mut self) {
        write!(self.stdout, "{}", termion::cursor::Hide).unwrap();
    }

    pub fn show_cursor(&mut self) {
        write!(self.stdout, "{}", termion::cursor::Show).unwrap();
    }

    pub fn flush(&mut self) {
        self.stdout.flush().unwrap();
    }

    pub fn write(&mut self, s: &str) {
        write!(self.stdout, "{}", s).unwrap();
    }

    // Returns the current terminal size as a tuple of (width, height, changed)
    // changed is true if the size changed since the last call to get_size
    pub fn get_size(&mut self) -> (i32, i32, bool) {
        let (width, height) = termion::terminal_size().unwrap();
        let (width, height) = (width as i32, height as i32);
        let mut changed = false;

        if width != self.width || height != self.height {
            self.width = width;
            self.height = height;
            changed = true;
        }

        (self.width, self.height, changed)
    }

    // Given a frame (Mat), writes it as a series of colored blocks
    pub fn write_frame(&mut self, frame: &Mat) {
        let data = frame.data_typed::<Point3_<u8>>().unwrap();
        let frame_width = frame.cols();
        let frame_height = frame.rows();

        let mut prev_color: String = String::from("");

        for (i, pixel) in data.iter().enumerate() {
            if (i % frame_width as usize == 0) && i != 0 {
                write!(self.stdout, "\n\r").unwrap();
            }

            let (b, g, r) = (pixel.x, pixel.y, pixel.z);
            let color = termion::color::Rgb(r, g, b).bg_string();

            // update color if it changed
            if color != prev_color {
                write!(self.stdout, "{}", color).unwrap();
                prev_color = color;
            }

            write!(self.stdout, "{}", " ").unwrap();
        }

        write!(self.stdout, "{}", termion::color::Reset.bg_str()).unwrap();
    }
}
