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
use std::{
    io::{self, Write},
    sync::{atomic, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use terminal::Terminal;
use ui::{handle_input_home_screen, render_contacts, wait_get_name};

// Minimum settings for camera
const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 30 as f64;
const FRAME_COMPRESSION_FACTOR: f64 = 0.5;

#[tokio::main]
async fn main() {
    let rtc_offerer_connection = PeerConnection::new(true).await.unwrap();
    let rtc_answerer_connection = PeerConnection::new(false).await.unwrap();

    let rtdb = RTDB::new();
    let mut terminal = Terminal::new();

    // ---------- Entering Name ----------
    println!("Enter your name: ");
    let self_name = wait_get_name(&rtdb).await;
    let user = User::new(self_name.clone());
    rtdb.add_or_update_user(&self_name, user).await.unwrap();

    // ---------- Home Page ----------

    let mut usernames: Vec<String> = vec![];
    let mut person_to_call = String::new();

    let mut begin = std::time::Instant::now();
    let mut contacts = rtdb.get_users().await;

    terminal.clear();
    loop {
        // Poll for firebase changes each second
        if begin.elapsed().as_secs() > 1 {
            begin = std::time::Instant::now();
            contacts = rtdb.get_users().await;
        }

        let new_usernames = contacts.keys().cloned().collect::<Vec<String>>();

        // If any update, rerender the contacts
        if new_usernames.len() != usernames.len() {
            usernames = new_usernames;
            terminal.clear();
            person_to_call.clear();
            render_contacts(&mut usernames, &self_name)
        }

        // returns true if we have a valid person to call
        if handle_input_home_screen(&usernames, &mut person_to_call) {
            break;
        }

        // Check if anyone is calling us (someone else's sending_call is our name)
        let potential_caller = contacts.iter().find(|(_k, v)| v.sending_call == self_name);

        // ---------- Call Handling ----------
        if let Some((caller_name, caller_data)) = potential_caller {
            println!("Receiving call from {}! Answering...", caller_name);
            let remote_offer = caller_data.offer.clone();
            let (remote_sdp, remote_candidates) = serde_json::from_str(&remote_offer).unwrap();

            rtc_answerer_connection
                .set_remote_description(remote_sdp)
                .await
                .unwrap();

            rtc_answerer_connection
                .add_remote_ice_candidates(remote_candidates)
                .await
                .unwrap();

            println!("Remote SDP and Candidates set!");

            // output answer and candidates for local peer
            let sdp = rtc_answerer_connection.create_answer().await.unwrap();
            rtc_answerer_connection
                .set_local_description(sdp.clone())
                .await
                .unwrap();
            while !rtc_answerer_connection
                .all_candidates_gathered
                .load(atomic::Ordering::SeqCst)
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            println!("Answer Created and Candidates gathered!");

            let candidates = rtc_answerer_connection.candidates.lock().unwrap().clone();
            let answer = serde_json::to_string(&(sdp, candidates)).unwrap();

            // update our user object with the answer
            let user = User {
                answer,
                receiving_call: caller_name.to_string(),
                ..User::new(self_name.clone())
            };
            rtdb.add_or_update_user(&self_name, user).await.unwrap();

            println!("Answer sent! Waiting for connection...");

            // and now just wait for connection/data channels to establish
            rtc_answerer_connection.wait_peer_connected().await;
            rtc_answerer_connection.wait_data_channels_open().await;

            // We are now in call with the caller. Update our user object to reflect this
            rtdb.add_or_update_user(
                &self_name,
                User {
                    in_call: caller_name.to_string(),
                    ..User::new(self_name.clone())
                },
            )
            .await
            .unwrap();

            // Once ready, we can start sending data
            call_loop(&rtdb, &self_name, &rtc_answerer_connection).await;
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    // ---------- Call Sending ----------

    // Generate offer
    println!("Generating offer...");
    let sdp = rtc_offerer_connection.create_offer().await.unwrap();
    rtc_offerer_connection
        .set_local_description(sdp.clone())
        .await
        .unwrap();

    while !rtc_offerer_connection
        .all_candidates_gathered
        .load(atomic::Ordering::SeqCst)
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("SDP Created and Candidates gathered! Passed the loop!");

    let candidates = rtc_offerer_connection.candidates.lock().unwrap().clone();

    // Serialize offer and candidates
    let offer = serde_json::to_string(&(sdp, candidates)).unwrap();

    // Set sending_call field of self to the person we want to call
    let self_data = User {
        offer,
        sending_call: person_to_call.clone(),
        ..User::new(self_name.clone())
    };
    rtdb.add_or_update_user(&self_name, self_data)
        .await
        .unwrap();

    println!(
        "Calling {} (sent offer)... Waiting for response...",
        person_to_call
    );

    // Wait for the person we are calling to send us an answer
    let mut answer;
    loop {
        let contacts = rtdb.get_users().await;
        answer = contacts.get(&person_to_call).unwrap().answer.clone();

        if answer != "" {
            break;
        }

        // sleep for 1 second
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    // We have received an answer
    println!("Received answer from {}! Connecting...", person_to_call);
    let (remote_sdp, remote_candidates) = serde_json::from_str(&answer).unwrap();
    rtc_offerer_connection
        .set_remote_description(remote_sdp)
        .await
        .unwrap();
    rtc_offerer_connection
        .add_remote_ice_candidates(remote_candidates)
        .await
        .unwrap();

    println!("Remote SDP and Candidates set! Waiting for connection...");

    // Wait for connection to establish
    rtc_offerer_connection.wait_peer_connected().await;
    rtc_offerer_connection.wait_data_channels_open().await;

    // Update our user object with the in_call field
    rtdb.add_or_update_user(
        &self_name,
        User {
            in_call: person_to_call.clone(),
            ..User::new(self_name.clone())
        },
    )
    .await
    .unwrap();

    call_loop(&rtdb, &self_name, &rtc_offerer_connection).await;
}

async fn call_loop(rtdb: &RTDB, self_name: &str, peer_connection: &PeerConnection) {
    // prompt for what index camera to use
    print!("Enter camera index: ");
    io::stdout().flush().unwrap();
    let mut camera_index_str = String::new();
    io::stdin().read_line(&mut camera_index_str).unwrap();
    let camera_index = camera_index_str.trim().parse().unwrap();

    let mut terminal = Terminal::new();
    let mut camera = Camera::new();
    let mut frame = Frame::new();
    let mut display_frame = Frame::new();

    camera.init(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS, camera_index);

    enable_raw_mode().unwrap();

    terminal.clear();
    terminal.hide_cursor();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    // frame capturing and sending loop
    let sending_bytes = Arc::new(atomic::AtomicUsize::new(0));
    let sending_bytes_read = Arc::clone(&sending_bytes);

    let label = if peer_connection.is_offerer {
        "offerer-send"
    } else {
        "answerer-send"
    }
    .to_string();
    let send_dc = peer_connection.get_data_channel(label).await.unwrap();

    tokio::spawn(async move {
        loop {
            let start = std::time::Instant::now();
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            assert!(camera.read_frame(frame.get_mut_ref()));
            frame.resize_frame(
                CAMERA_WIDTH * FRAME_COMPRESSION_FACTOR,
                CAMERA_HEIGHT * FRAME_COMPRESSION_FACTOR,
                true,
            );

            let frame = &bytes::Bytes::from(frame.get_bytes());
            let timestamp_bytes = timestamp.to_be_bytes();

            let mut payload = BytesMut::with_capacity(frame.len() + timestamp_bytes.len());
            payload.extend_from_slice(&frame);
            payload.extend_from_slice(&timestamp_bytes);
            sending_bytes.store(payload.len(), atomic::Ordering::SeqCst);

            if send_dc.send(&payload.freeze()).await.is_err() {
                break;
            }

            // at most, send at 30fps
            let elapsed = start.elapsed();
            if elapsed < Duration::from_millis(1000 / 30) {
                tokio::time::sleep(Duration::from_millis(1000 / 30) - elapsed).await;
            }
        }
    });

    // frame receiving and rendering loop
    loop {
        let loop_start = std::time::Instant::now();
        // If q pressed, gracefully quit
        if event::poll(std::time::Duration::from_millis(1)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Char('q') {
                    graceful_exit(&rtdb, self_name, &mut terminal).await;
                }
            }
        }

        // receive data from data channel and play it
        let on_message_rx = peer_connection.on_message_rx.clone();
        let mut on_message_rx = on_message_rx.lock().unwrap();
        let payload = on_message_rx.recv().await;
        if payload.is_none() {
            graceful_exit(&rtdb, self_name, &mut terminal).await;
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

        // some processing before showing the frame
        let (terminal_width, terminal_height, size_changed) = terminal.get_size();
        display_frame.resize_frame(terminal_width as f64, (terminal_height - 1) as f64, false);
        display_frame.change_color_depth(32);

        // during size changes, don't render (to avoid artifacts)
        if !size_changed {
            terminal.goto_topleft();
            terminal.write_frame(display_frame.get_frame());
        } else {
            terminal.clear();
        }

        let stats = format!(
            "latency (s): {:.1} | send/receiving {:.0}/{:.0} kb/s | pixels: {} ({}x{}) | fps: {:.0}",
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
}

async fn graceful_exit(rtdb: &RTDB, self_name: &str, terminal: &mut Terminal) {
    rtdb.remove_user(self_name).await.unwrap();
    disable_raw_mode().unwrap();
    terminal.show_cursor();
    std::process::exit(0);
}
