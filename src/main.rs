mod app;
mod devices;
mod frame;
mod peer_connection;
mod rtdb;
mod schemas;
mod stats;
mod tui;

use anyhow::anyhow;
use app::App;
use bytes::BytesMut;
use crossterm::event;
use devices::camera::{self, Camera, CAMERA_HEIGHT, CAMERA_WIDTH};
use frame::Frame;
use peer_connection::PeerConnection;
use rtdb::RTDB;
use schemas::user::User;
use simple_log::{error, LogConfigBuilder};
use std::{
    io::{self, Write},
    sync::{atomic, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

// Minimum settings for camera
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
    App::default().run(&mut terminal, &self_name).await?;
    tui::restore()?;

    Ok(())
}

pub async fn wait_get_unique_name(rtdb: &RTDB) -> anyhow::Result<String> {
    let mut self_name = String::new();
    loop {
        io::stdin().read_line(&mut self_name)?;
        self_name = self_name.trim().to_string();
        if self_name == "" {
            print!("Name cannot be empty. Try entering a different name: ");
            io::stdout().flush()?;
            continue;
        }

        // no numbers as name. This is to avoid confusion with firebase making strings into numbers
        if self_name.chars().all(char::is_numeric) {
            print!("Name cannot be all numbers. Try entering a different name: ");
            io::stdout().flush()?;
            self_name.clear();
            continue;
        }

        let usernames = rtdb.get_usernames().await;
        match usernames.contains(&self_name) {
            true => {
                print!("User already exists. Try entering a different name: ");
                io::stdout().flush()?;
                self_name.clear();
            }
            false => break,
        }
    }

    Ok(self_name)
}

// ---------- Handle Incoming Call ----------
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

    call_loop(&rtc_connection).await;

    Ok(())
}

// ---------- Handle Sending Call ----------
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
        tokio::time::sleep(Duration::from_millis(500)).await;
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

    call_loop(&rtc_connection).await;

    Ok(())
}

// ---------- Call Loop ----------
async fn call_loop(rtc_connection: &PeerConnection) -> anyhow::Result<()> {
    let mut terminal = tui::init()?;
    let mut display_frame = Frame::new();
    terminal.clear();

    let mut frame_times = vec![];

    let sending_bytes = Arc::new(atomic::AtomicUsize::new(0));
    let sending_bytes_read = Arc::clone(&sending_bytes);

    let send_dc_label = &format!("{}-send", rtc_connection.id);
    let send_dc = rtc_connection
        .get_data_channel(send_dc_label)
        .await
        .expect("Data channel should exist");

    // ---------- Frame Sending Loop ----------
    tokio::spawn(async move {
        let mut cam = Camera::new();

        let mut frame = Frame::new();

        loop {
            let start = std::time::Instant::now();
            let timestamp = timestamp();

            match cam.read_frame(frame.get_mut_ref()) {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed reading camera frame. Ending loop: {:?}", e);
                    break;
                }
            }

            frame
                .resize_frame(
                    (640 as f64) * FRAME_COMPRESSION_FACTOR,
                    (480 as f64) * FRAME_COMPRESSION_FACTOR,
                    false,
                )
                .unwrap();

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
    let mut tsize = terminal.size()?;
    'frame_rec_loop: loop {
        let loop_start = std::time::Instant::now();

        // If Esc pressed, gracefully quit
        if event::poll(std::time::Duration::from_millis(0)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Esc {
                    break 'frame_rec_loop;
                }
            }
        }

        // Receive payload from data channel
        let on_message_rx = rtc_connection.on_message_rx.clone();
        let mut on_message_rx = on_message_rx.lock().unwrap();
        let payload = match on_message_rx.recv().await {
            Some(payload) => Some(payload),
            None => {
                break 'frame_rec_loop;
            }
        };

        // Unpack payload and calculate stats
        let payload = payload.unwrap().data;
        let (frame, timestamp_bytes) = payload.split_at(payload.len() - 8);

        let receiving_bytes = payload.len();
        let timestamp_ = u64::from_be_bytes(timestamp_bytes.try_into().unwrap());
        let latency = timestamp() - timestamp_;

        // Clear terminal if size changed (to avoid artifacts)
        if terminal.size()? != tsize {
            terminal.clear();
            tsize = terminal.size()?;
        }

        // Render frame to terminal
        display_frame.load_bytes(frame.to_vec());
        display_frame.resize_frame(tsize.width as f64, (tsize.height - 1) as f64, false);
        display_frame.write_to_terminal();

        // Print stats
        let stats = format!(
            "latency: {:.2} s | send/recv {:.0}/{:.0} kb/s | res: {}x{} ({} pix) | fps: {}",
            latency as f64 / 1000.0,
            sending_bytes_read.load(atomic::Ordering::SeqCst) as f64 / 1000.0,
            receiving_bytes as f64 / 1000.0,
            display_frame.width(),
            display_frame.height(),
            display_frame.num_pixels(),
            frame_times.len(),
        );

        // Render stats at bottom right corner if string fits
        if tsize.width >= stats.len() as u16 {
            write!(
                io::stdout(),
                "{}{}",
                crossterm::cursor::MoveTo(
                    tsize.width as u16 - stats.len() as u16,
                    tsize.height as u16
                ),
                stats
            )?;
            io::stdout().flush()?;
        };

        // Cap fps to 30
        if loop_start.elapsed() < Duration::from_millis(1000 / 30) {
            tokio::time::sleep(Duration::from_millis(1000 / 30) - loop_start.elapsed()).await;
        }

        // Calculate fps based on moving frame rate every second
        frame_times.push(loop_start.elapsed());
        while frame_times.iter().sum::<Duration>() >= Duration::from_secs(1) {
            frame_times.swap_remove(0);
        }
    }

    Ok(())
}
