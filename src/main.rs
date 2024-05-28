mod devices;
mod frame;
mod rtc;
mod rtdb;
mod schemas;
mod stats;
mod terminal;
mod ui;

use bytes::BytesMut;
use crossterm::{
    event,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use devices::camera::Camera;
use frame::Frame;
use rtc::PeerConnection;
use rtdb::RTDB;
use schemas::user::User;
use simple_log::{error, LogConfigBuilder};
use std::{
    sync::{atomic, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use terminal::Terminal;
use ui::{handle_homescreen_input, render_homescreen, wait_get_name};

// Minimum settings for camera
const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 30 as f64;
const FRAME_COMPRESSION_FACTOR: f64 = 0.5;

fn timstamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::main]
async fn main() {
    // Initialize logging
    let config = LogConfigBuilder::builder()
        .path(&format!("./logs/{}.log", timstamp()))
        .size(1 * 100)
        .roll_count(10)
        .time_format("%Y-%m-%d %H:%M:%S")
        .level("warning")
        .output_file()
        .build();

    match simple_log::new(config) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error initializing logging: {:?}", e);
            return;
        }
    }

    // Initialize PeerConnections
    let rtc_connection = match PeerConnection::new().await {
        Ok(connection) => connection,
        Err(e) => {
            error!("Error creating PeerConnection: {:?}", e);
            return;
        }
    };

    // Initialize Firebase RTDB connection and Terminal
    let rtdb = RTDB::new();
    let mut terminal = Terminal::new();

    // ---------- Entering Name Screen ----------
    println!("Enter your name: ");
    let self_name = match wait_get_name(&rtdb).await {
        Ok(name) => name,
        Err(e) => {
            error!("Error getting name: {:?}", e);
            return;
        }
    };

    let user = User::new(self_name.clone());

    rtdb.add_or_update_user(&self_name, user).await.unwrap();
    terminal.clear();

    // ---------- Home Screen ----------

    let mut usernames: Vec<String> = vec![];
    let mut person_to_call = String::new();

    let mut begin = std::time::Instant::now();
    let mut contacts = rtdb.get_users().await;

    loop {
        // Poll for firebase changes each second
        if begin.elapsed().as_secs() > 1 {
            begin = std::time::Instant::now();
            contacts = rtdb.get_users().await;
        }

        // Poll for user input
        if event::poll(std::time::Duration::from_millis(50)).unwrap() {
            if handle_homescreen_input(&usernames, &mut person_to_call) {
                break;
            }
        }

        let new_usernames = contacts.keys().cloned().collect::<Vec<String>>();

        // If any update, rerender the contacts
        if new_usernames.len() != usernames.len() {
            terminal.clear();
            person_to_call.clear();
            usernames = new_usernames;
            render_homescreen(&mut usernames, &self_name)
        }

        // Check if anyone is calling us (someone else's sending_call is our name)
        let potential_caller = contacts.iter().find(|(_k, v)| v.sending_call == self_name);

        // ---------- Call Handling ----------
        if let Some((caller_name, caller_data)) = potential_caller {
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

            let answer = serde_json::to_string(&(sd, candidates))
                .expect("Components should be serializable");

            rtdb.add_or_update_user(
                &self_name,
                User {
                    answer,
                    receiving_call: caller_name.to_string(),
                    ..User::new(self_name.clone())
                },
            )
            .await
            .unwrap();

            println!("Answer sent! Waiting for connection...");

            rtc_connection.wait_peer_connected().await;
            rtc_connection.wait_data_channels_open().await;

            rtdb.add_or_update_user(
                &self_name,
                User {
                    in_call: caller_name.to_string(),
                    ..User::new(self_name.clone())
                },
            )
            .await
            .unwrap();

            call_loop(&rtdb, &self_name, &rtc_connection).await;
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    // ---------- Call Sending ----------

    println!("Calling {}...", person_to_call);
    let sdp = rtc_connection.create_offer().await.unwrap();

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
            sending_call: person_to_call.clone(),
            ..User::new(self_name.clone())
        },
    )
    .await
    .unwrap();

    let mut answer = String::new();
    while answer == "" {
        let contacts = rtdb.get_users().await;
        answer = contacts
            .get(&person_to_call)
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
            in_call: person_to_call.clone(),
            ..User::new(self_name.clone())
        },
    )
    .await
    .unwrap();

    call_loop(&rtdb, &self_name, &rtc_connection).await;
}

// ---------- Call Loop ----------
async fn call_loop(rtdb: &RTDB, self_name: &str, rtc_connection: &PeerConnection) {
    let mut terminal = Terminal::new();
    let mut display_frame = Frame::new();

    enable_raw_mode().unwrap();

    terminal.clear();
    terminal.hide_cursor();

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
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

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

        // If q pressed, gracefully quit
        if event::poll(std::time::Duration::from_millis(0)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Char('q') {
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

        let timestamp = u64::from_be_bytes(timestamp_bytes.try_into().unwrap());
        let latency = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - timestamp;

        display_frame.load_bytes(frame.to_vec());

        let (terminal_width, terminal_height, size_changed) = terminal.get_size();
        match size_changed {
            true => {
                terminal.clear();
            }
            false => {
                display_frame.resize_frame(
                    terminal_width as f64,
                    (terminal_height - 1) as f64,
                    false,
                );
                terminal.goto_topleft();
                terminal.write_frame(display_frame.get_frame());
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

        terminal.write_to_bottomright(&stats);

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

    graceful_exit(&rtdb, self_name, &mut terminal).await;
}

async fn graceful_exit(rtdb: &RTDB, self_name: &str, terminal: &mut Terminal) {
    rtdb.remove_user(self_name).await;
    disable_raw_mode().unwrap();
    terminal.show_cursor();
    std::process::exit(0);
}
