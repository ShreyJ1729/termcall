mod camera;
mod microphone;
mod speaker;
mod stats;
mod terminal;
mod webrtc_handler;

use camera::Camera;
use just_webrtc::{
    types::{ICECandidate, SessionDescription},
    DataChannelExt, PeerConnectionExt, SimpleLocalPeerConnection,
};
use microphone::Microphone;
use speaker::Speaker;
use stats::get_memory_usage;
use terminal::Terminal;
use text_io::read;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc_handler::WebRTC_Handler;

const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 24 as f64;

#[tokio::main]
async fn main() {
    let mut camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS).unwrap();
    let mut microphone = Microphone::new();
    let mut speaker = Speaker::new();
    let mut terminal = Terminal::new();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    // for cleaner display
    terminal.clear();
    terminal.hide_cursor();

    // https://crates.io/crates/just-webrtc

    let mut local_peer_connection = SimpleLocalPeerConnection::build(false).await.unwrap();
    let offer = local_peer_connection.get_local_description().await.unwrap();
    let candidates = local_peer_connection
        .collect_ice_candidates()
        .await
        .unwrap();

    println!("Offer: {}", offer.sdp);
    println!("Candidates: {:?}", candidates);

    // pause for 60 seconds to allow for manual testing
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;

    // listen for exit signal (ctrl+c) - once pressed, bring back cursor and exit
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        Terminal::new().show_cursor();
        println!("Exiting...");
        std::process::exit(0);
    });

    loop {
        let (terminal_width, terminal_height, size_changed) = terminal.get_size();

        assert!(camera.read_frame());
        camera.resize_frame(terminal_width as f64, (terminal_height - 1) as f64, true);
        camera.change_color_depth(24);

        // clear terminal if size changes (to avoid artifacts)
        if size_changed {
            terminal.clear();
        }

        terminal.goto_topleft();

        terminal.write_frame(camera.get_frame_mirrored());

        let stats = format!(
            "mem usage: {:.0}MB | pixels: {} ({}x{}) | fps: {:.0}",
            get_memory_usage(),
            camera.get_frame_num_pixels(),
            camera.get_frame_width(),
            camera.get_frame_height(),
            frame_count as f64 / begin.elapsed().as_secs_f64()
        );

        terminal.write_to_bottomright(&stats);

        // calculate fps based on moving frame rate every second
        if begin.elapsed().as_secs() > 1 {
            frame_count = 0;
            begin = std::time::Instant::now();
        }
        frame_count += 1;
    }
}
