mod devices;
mod rtdb;
mod schemas;
mod stats;
mod terminal;

use bytes::BytesMut;
use crossterm::{
    event,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use devices::{camera::Camera, microphone::Microphone, speaker::Speaker};
use firebase_rs::Firebase;
use just_webrtc::{
    platform::{Channel, PeerConnection},
    DataChannelExt, PeerConnectionExt, SimpleLocalPeerConnection, SimpleRemotePeerConnection,
};
use schemas::user::User;
use stats::get_memory_usage;
use std::{
    fs::File,
    io::{self, stdin, Write},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use terminal::Terminal;
use tokio::sync::Mutex;

const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 30 as f64;

#[tokio::main]
async fn main() {
    let firebase = Firebase::new(rtdb::DATABASE_URL).unwrap();

    let mut local_peer_connection = SimpleLocalPeerConnection::build(false).await.unwrap();
    let mut terminal = Terminal::new();

    // ---------- Entering Name ----------
    println!("Enter your name: ");

    let mut self_name = String::new();

    loop {
        stdin()
            .read_line(&mut self_name)
            .expect("Failed to read line");
        self_name = self_name.trim().to_string();

        let usernames = rtdb::get_usernames(&firebase).await;
        if usernames.contains(&self_name) {
            println!("User already exists. Try entering a different name: ");
            self_name.clear();
            continue;
        }
        break;
    }

    // adding user to firebase

    let data = User::new(self_name.to_string());
    rtdb::add_or_update_user(&firebase, &self_name, data)
        .await
        .unwrap();

    // ---------- Home Page ----------

    let mut usernames: Vec<String> = vec![];
    let mut person_to_call = String::new();

    terminal.clear();

    loop {
        // Poll for firebase changes each cycle
        let contacts = rtdb::get_users(&firebase).await;
        let new_usernames = contacts.keys().cloned().collect::<Vec<String>>();

        // If any update, rerender the contacts
        if new_usernames.len() != usernames.len() {
            usernames = new_usernames;
            terminal.clear();
            person_to_call.clear();
            println!(
                    "Welcome, {}! This is your dashboard. If anyone calls you, you'll get a notification here. If you want to call someone, enter their name below.\nNames are case sensitive\n",
                    self_name
                );
            usernames.sort();
            usernames
                .iter()
                .filter(|username| username != &&self_name)
                .enumerate()
                .for_each(|(i, contact)| {
                    println!("{}: {}", i, contact);
                });
        }

        // Poll for user input
        if event::poll(std::time::Duration::from_millis(50)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Enter {
                    if usernames.contains(&person_to_call) {
                        break;
                    }
                    println!("That person is not in your contacts. Try again.");
                    person_to_call.clear();
                } else if event.code == event::KeyCode::Backspace {
                    if person_to_call.len() > 0 {
                        person_to_call.pop();
                    }
                } else if let event::KeyCode::Char(c) = event.code {
                    person_to_call.push(c);
                }
            }
        }

        // Check if anyone is calling us (someone else's sending_call is our name)
        let potential_caller = contacts.iter().find(|(_k, v)| v.sending_call == self_name);

        // If they are, send an answer back

        // ---------- Call Handling ----------
        if let Some((caller_name, caller_data)) = potential_caller {
            println!("Receiving call from {}! Answering...", caller_name);
            let remote_offer = caller_data.offer.clone();
            let (remote_sdp, remote_candidates) = serde_json::from_str(&remote_offer).unwrap();

            let mut remote_peer_connection =
                SimpleRemotePeerConnection::build(remote_sdp).await.unwrap();

            remote_peer_connection
                .add_ice_candidates(remote_candidates)
                .await
                .unwrap();

            // output answer and candidates for local peer
            let sdp = remote_peer_connection
                .get_local_description()
                .await
                .unwrap();
            let candidates = remote_peer_connection
                .collect_ice_candidates()
                .await
                .unwrap();

            // ... send the answer and the candidates back to Peer A via external signalling implementation ...
            let answer = (sdp, candidates);
            let answer = serde_json::to_string(&answer).unwrap();

            // update our user object with the answer
            let user = User {
                answer,
                receiving_call: caller_name.to_string(),
                ..User::new(self_name.clone())
            };
            rtdb::add_or_update_user(&firebase, &self_name, user)
                .await
                .unwrap();

            println!("Answer sent! Waiting for connection...");

            // and now just wait for connection/data channels to establish
            remote_peer_connection.wait_peer_connected().await;
            let remote_channel = remote_peer_connection.receive_channel().await.unwrap();
            remote_channel.wait_ready().await;

            // We are now in call with the caller. Update our user object to reflect this
            rtdb::add_or_update_user(
                &firebase,
                &self_name,
                User {
                    in_call: caller_name.to_string(),
                    ..User::new(self_name.clone())
                },
            )
            .await
            .unwrap();

            // Once ready, we can start sending data
            call_loop(
                &firebase,
                &self_name,
                caller_name,
                remote_peer_connection,
                remote_channel,
            )
            .await;
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    // ---------- Call Sending ----------

    // Generate offer
    println!("Generating offer...");
    let sdp = local_peer_connection.get_local_description().await.unwrap();
    let candidates = local_peer_connection
        .collect_ice_candidates()
        .await
        .unwrap();

    // Serialize offer and candidates
    let offer = serde_json::to_string(&(sdp, candidates)).unwrap();

    // Set sending_call field of self to the person we want to call
    let self_data = User {
        offer,
        sending_call: person_to_call.clone(),
        ..User::new(self_name.clone())
    };
    rtdb::add_or_update_user(&firebase, &self_name, self_data)
        .await
        .unwrap();

    println!(
        "Calling {} (send offer)... Waiting for response...",
        person_to_call
    );

    // Wait for the person we are calling to send us an answer
    let mut answer;
    loop {
        let contacts = rtdb::get_users(&firebase).await;
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
    local_peer_connection
        .set_remote_description(remote_sdp)
        .await
        .unwrap();
    local_peer_connection
        .add_ice_candidates(remote_candidates)
        .await
        .unwrap();

    // Wait for connection to establish
    local_peer_connection.wait_peer_connected().await;
    let local_channel = local_peer_connection.receive_channel().await.unwrap();
    local_channel.wait_ready().await;

    // Update our user object with the in_call field
    rtdb::add_or_update_user(
        &firebase,
        &self_name,
        User {
            in_call: person_to_call.clone(),
            ..User::new(self_name.clone())
        },
    )
    .await
    .unwrap();

    call_loop(
        &firebase,
        &self_name,
        &person_to_call,
        local_peer_connection,
        local_channel,
    )
    .await;
}

async fn call_loop(
    firebase: &Firebase,
    self_name: &str,
    peer_name: &str,
    rtc_connection: PeerConnection,
    data_channel: Channel,
) {
    // prompt for what index camera to use
    print!("Enter camera index: ");
    io::stdout().flush().unwrap();
    let mut camera_index_str = String::new();
    io::stdin().read_line(&mut camera_index_str).unwrap();
    let camera_index = camera_index_str.trim().parse().unwrap();

    let mut terminal = Terminal::new();
    let mut camera_read = Camera::new().unwrap();
    let mut camera_math = Camera::new().unwrap();
    camera_read.init(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS, camera_index);

    let mut microphone = Microphone::new();
    let mut speaker = Speaker::new();
    let data_channel = Arc::new(Mutex::new(data_channel));

    enable_raw_mode().unwrap();

    terminal.clear();
    terminal.hide_cursor();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    // frame capturing and sending loop
    let data_channel_clone = Arc::clone(&data_channel);

    tokio::spawn(async move {
        loop {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            assert!(camera_read.read_frame());

            let w = camera_read.get_frame_width();
            let h = camera_read.get_frame_height();

            // downsample frame for easier transmission
            camera_read.resize_frame(w as f64 * 0.25, h as f64 * 0.25, false);

            // convert mat to bytes and send over data channel
            let payload = &bytes::Bytes::from(camera_read.mat_to_bytes());
            let timestamp_bytes = timestamp.to_be_bytes();

            let mut combined = BytesMut::with_capacity(payload.len() + timestamp_bytes.len());
            combined.extend_from_slice(&payload);
            combined.extend_from_slice(&timestamp_bytes);

            let data_channel = data_channel_clone.lock().await;

            if data_channel.send(&combined.freeze()).await.is_err() {
                break;
            }
        }
    });

    // frame receiving and rendering loop
    loop {
        // If q pressed, gracefully quit
        if event::poll(std::time::Duration::from_millis(1)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Char('q') {
                    // remove self from firebase db, restore terminal and exit
                    rtdb::remove_user(&firebase, self_name).await.unwrap();
                    rtdb::remove_user(&firebase, peer_name).await.unwrap();
                    disable_raw_mode().unwrap();
                    terminal.show_cursor();
                    std::process::exit(0);
                }
            }
        }

        // If self or peer no longer exist in firebase, exit (this means peer has hung up)
        // Since this is somewhat expensive, we only check at most every second
        if frame_count % 30 == 0 {
            let users = rtdb::get_users(&firebase).await;
            if !users.contains_key(self_name) || !users.contains_key(peer_name) {
                disable_raw_mode().unwrap();
                terminal.show_cursor();
                std::process::exit(0);
            }
        }

        // receive data from data channel and play it
        let mut data_channel = data_channel.lock().await;

        let combined = data_channel.receive().await;
        if combined.is_err() {
            break;
        }
        let combined = combined.unwrap();
        let (payload, timestamp_bytes) = combined.split_at(combined.len() - 8);

        let timestamp = u64::from_be_bytes(timestamp_bytes.try_into().unwrap());
        let latency = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - timestamp;

        camera_math.save_bytes_to_mat(payload.to_vec());

        // some processing before showing the frame
        let (terminal_width, terminal_height, size_changed) = terminal.get_size();
        camera_math.resize_frame(terminal_width as f64, (terminal_height - 1) as f64, false);
        camera_math.change_color_depth(24);

        // during size changes, don't render (to avoid artifacts)
        if !size_changed {
            terminal.goto_topleft();
            terminal.write_frame(camera_math.get_frame());
        } else {
            terminal.clear();
        }

        let stats = format!(
            "latency (s): {:.1} | pixels: {} ({}x{}) | fps: {:.0}",
            latency as f64 / 1000.0,
            camera_math.get_frame_num_pixels(),
            camera_math.get_frame_width(),
            camera_math.get_frame_height(),
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
