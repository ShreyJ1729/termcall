mod app;
mod devices;
mod frame;
mod frame_writer;
mod rtc;
mod rtdb;
mod schemas;
mod stats;
mod tui;
mod ui;

use anyhow::anyhow;
use app::App;
use bytes::BytesMut;
use crossterm::event;
use devices::camera::Camera;
use frame::Frame;
use frame_writer::FrameWriter;
use rtc::PeerConnection;
use rtdb::RTDB;
use schemas::user::User;
use simple_log::{error, info, LogConfigBuilder};
use std::{
    io::{self, Write},
    sync::{atomic, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use ui::wait_get_unique_name;

// Minimum settings for camera
const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 30 as f64;
const FRAME_COMPRESSION_FACTOR: f64 = 0.5;

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn init_logging() -> anyhow::Result<(), String> {
    let config = LogConfigBuilder::builder()
        .path(&format!("./logs/{}.log", timestamp()))
        .size(1 * 100)
        .roll_count(10)
        .time_format("%Y-%m-%d %H:%M:%S")
        .level("warning")
        .output_file()
        .build();

    simple_log::new(config)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging().map_err(|e| anyhow!(e))?;
    let rtdb = RTDB::new();

    // ---------- Entering Name Screen ----------
    print!("Enter your name: ");
    io::stdout().flush()?;
    let self_name = wait_get_unique_name(&rtdb).await?;
    let user = User::new(self_name.clone());
    rtdb.add_or_update_user(&self_name, user).await?;

    // ---------- Main App Loop ----------
    let mut terminal = tui::init()?;
    let app_result = App::default().run(&mut terminal, &self_name).await;

    Ok(())
}

async fn handle_incoming_call(
    self_name: &str,
    caller_data: &User,
    rtdb: &RTDB,
    rtc_connection: &PeerConnection,
) -> anyhow::Result<()> {
    let caller_name = caller_data.name.clone();
    println!("Answering call from {}...", caller_name);
    let remote_offer = caller_data.offer.clone();
    let (remote_sd, remote_candidates) =
        serde_json::from_str(&remote_offer).expect("Remote offer should be valid JSON");

    rtc_connection
        .set_remote_description(remote_sd)
        .await
        .expect("Remote session description should be valid");

    rtc_connection
        .add_remote_ice_candidates(remote_candidates)
        .await
        .expect("Remote ice candidates should be valid");

    let sd = rtc_connection
        .create_answer()
        .await
        .expect("peer connection offer should be set");

    rtc_connection
        .set_local_description(sd.clone())
        .await
        .expect("Local session description should be valid");

    rtc_connection.wait_ice_candidates_gathered().await;
    let candidates = rtc_connection.get_ice_candidates().await;

    let answer =
        serde_json::to_string(&(sd, candidates)).expect("Components should be serializable");

    rtdb.add_or_update_user(
        &self_name,
        User {
            answer,
            receiving_call: caller_name.to_string(),
            ..User::new(self_name.to_string())
        },
    )
    .await?;

    println!("Answer sent! Waiting for connection...");

    rtc_connection.wait_peer_connected().await;
    rtc_connection.wait_data_channels_open().await;

    rtdb.add_or_update_user(
        &self_name,
        User {
            in_call: caller_name.to_string(),
            ..User::new(self_name.to_string())
        },
    )
    .await?;

    call_loop(&rtdb, &self_name, &rtc_connection).await;

    Ok(())
}

async fn handle_sending_call(
    self_name: &str,
    person_to_call: &str,
    rtdb: &RTDB,
    rtc_connection: &PeerConnection,
) -> anyhow::Result<()> {
    println!("Calling {}...", person_to_call);
    let sdp = rtc_connection.create_offer().await?;

    rtc_connection
        .set_local_description(sdp.clone())
        .await
        .expect("Local sd should be valid");

    rtc_connection.wait_ice_candidates_gathered().await;
    let candidates = rtc_connection.get_ice_candidates().await;

    let offer =
        serde_json::to_string(&(sdp, candidates)).expect("Components should be serializable");

    rtdb.add_or_update_user(
        &self_name,
        User {
            offer,
            sending_call: person_to_call.to_string(),
            ..User::new(self_name.to_string())
        },
    )
    .await?;

    let mut answer = String::new();
    while answer == "" {
        let contacts = rtdb.get_users().await;
        answer = contacts
            .get(person_to_call)
            .expect("Peer data should be in rtdb")
            .answer
            .clone();
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    println!("{} answered! Connecting...", person_to_call);
    let (remote_sd, remote_candidates) =
        serde_json::from_str(&answer).expect("Answer should be valid JSON");

    rtc_connection
        .set_remote_description(remote_sd)
        .await
        .expect("Remote session description should be valid");
    rtc_connection
        .add_remote_ice_candidates(remote_candidates)
        .await
        .expect("Remote ice candidates should be valid");

    rtc_connection.wait_peer_connected().await;
    rtc_connection.wait_data_channels_open().await;

    rtdb.add_or_update_user(
        &self_name,
        User {
            in_call: person_to_call.to_string(),
            ..User::new(self_name.to_string())
        },
    )
    .await?;

    call_loop(&rtdb, &self_name, &rtc_connection).await;

    Ok(())
}

// ---------- Call Loop ----------
async fn call_loop(rtdb: &RTDB, self_name: &str, rtc_connection: &PeerConnection) {
    let mut frame_writer = FrameWriter::new();
    let mut display_frame = Frame::new();

    frame_writer.clear();
    frame_writer.hide_cursor();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    let sending_bytes = Arc::new(atomic::AtomicUsize::new(0));
    let sending_bytes_read = Arc::clone(&sending_bytes);

    let send_dc_label = &format!("{}-send", rtc_connection.id);

    let send_dc = rtc_connection
        .get_data_channel(send_dc_label)
        .await
        .expect(format!("Data channel {} should exist", send_dc_label).as_str());

    // ---------- Frame Sending Loop ----------
    tokio::spawn(async move {
        let mut camera = Camera::new();
        let mut frame = Frame::new();

        match camera.init(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS, 0) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed initializing camera. Ending loop: {:?}", e);
                return;
            }
        }

        loop {
            let start = std::time::Instant::now();
            let timestamp = timestamp();

            match camera.read_frame(frame.get_mut_ref()) {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed reading camera frame. Ending loop: {:?}", e);
                    break;
                }
            }

            frame.resize_frame(
                CAMERA_WIDTH * FRAME_COMPRESSION_FACTOR,
                CAMERA_HEIGHT * FRAME_COMPRESSION_FACTOR,
                false,
            );

            let frame = frame.get_bytes();
            let timestamp_bytes = timestamp.to_be_bytes();

            let mut payload = BytesMut::with_capacity(frame.len() + timestamp_bytes.len());
            payload.extend_from_slice(&frame);
            payload.extend_from_slice(&timestamp_bytes);
            sending_bytes.store(payload.len(), atomic::Ordering::SeqCst);

            if send_dc.send(&payload.freeze()).await.is_err() {
                error!("Failed sending frame on data channel. Ending loop.");
                break;
            }

            // Cap sending rate at 30fps
            let elapsed = start.elapsed();
            if elapsed < Duration::from_millis(1000 / 30) {
                tokio::time::sleep(Duration::from_millis(1000 / 30) - elapsed).await;
            }
        }
    });

    // ---------- Frame Receiving/Rendering Loop ----------
    loop {
        let loop_start = std::time::Instant::now();

        // If Esc pressed, gracefully quit
        if event::poll(std::time::Duration::from_millis(0)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Esc {
                    break;
                }
            }
        }

        // receive data from data channel and play it
        let on_message_rx = rtc_connection.on_message_rx.clone();
        let mut on_message_rx = on_message_rx.lock().unwrap();

        let payload = on_message_rx.recv().await;
        if payload.is_none() {
            break;
        }

        let payload = payload.unwrap().data;
        let receiving_bytes = payload.len();
        let (frame, timestamp_bytes) = payload.split_at(payload.len() - 8);

        let timestamp_ = u64::from_be_bytes(timestamp_bytes.try_into().unwrap());
        let latency = timestamp() - timestamp_;

        display_frame.load_bytes(frame.to_vec());

        let (terminal_width, terminal_height, size_changed) = frame_writer.get_size();
        match size_changed {
            true => {
                frame_writer.clear();
            }
            false => {
                display_frame.resize_frame(
                    terminal_width as f64,
                    (terminal_height - 1) as f64,
                    false,
                );
                frame_writer.goto_topleft();
                frame_writer.write_frame(display_frame.get_frame());
            }
        }

        let stats = format!(
            "latency: {:.2} s | send/receiving {:.0}/{:.0} kb/s | pixels: {} ({}x{}) | fps: {:.0}",
            latency as f64 / 1000.0,
            sending_bytes_read.load(atomic::Ordering::SeqCst) as f64 / 1000.0,
            receiving_bytes as f64 / 1000.0,
            display_frame.num_pixels(),
            display_frame.width(),
            display_frame.height(),
            frame_count * 1000 / (begin.elapsed().as_millis() + 1)
        );

        frame_writer.write_to_bottomright(&stats);

        // calculate fps based on moving frame rate every second
        if begin.elapsed().as_secs() > 1 {
            frame_count = 0;
            begin = std::time::Instant::now();
        }

        frame_count += 1;

        // at most, render at 30fps
        let elapsed = loop_start.elapsed();
        if elapsed < Duration::from_millis(1000 / 30) {
            tokio::time::sleep(Duration::from_millis(1000 / 30) - elapsed).await;
        }
    }

    let on_close_rx = rtc_connection.on_close_rx.clone();
    let mut on_close_rx = on_close_rx.lock().unwrap();

    rtdb.remove_user(self_name).await;
    rtc_connection.close().await;
    tui::restore().expect("Failed to restore terminal");
}
