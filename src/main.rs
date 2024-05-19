mod devices;
mod rtdb;
mod schemas;
mod stats;
mod terminal;

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
use opencv::videoio::{self, VideoCaptureTraitConst};
use schemas::user::User;
use stats::get_memory_usage;
use std::{
    collections::HashMap,
    io::{self, stdin, Read, Write},
    sync::mpsc,
    thread,
    time::Duration,
};
use terminal::Terminal;

const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 30 as f64;

#[tokio::main]
async fn main() {
    let firebase = Firebase::new(rtdb::DATABASE_URL).unwrap();
    let rtdb_stream = firebase.with_realtime_events().unwrap();

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
        let potential_caller = contacts.iter().find(|(k, v)| v.sending_call == self_name);

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
            let mut remote_channel = remote_peer_connection.receive_channel().await.unwrap();
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
    let mut local_channel = local_peer_connection.receive_channel().await.unwrap();
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
    mut data_channel: Channel,
) {
    let mut terminal = Terminal::new();
    let mut camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS).unwrap();
    let mut microphone = Microphone::new();
    let mut speaker = Speaker::new();

    // pause for 10 seconds to allow for connection to establish
    tokio::time::sleep(Duration::from_secs(10)).await;

    enable_raw_mode().unwrap();

    terminal.clear();
    terminal.hide_cursor();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

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

        let (terminal_width, terminal_height, size_changed) = terminal.get_size();

        assert!(camera.read_frame());
        camera.resize_frame(terminal_width as f64, (terminal_height - 1) as f64, true);
        camera.change_color_depth(24);

        // convert mat to bytes and send over data channel
        let payload = &bytes::Bytes::from(camera.mat_to_bytes());
        data_channel.send(payload).await.unwrap();

        // receive data from data channel and play it
        let payload = data_channel.receive().await.unwrap();
        camera.save_bytes_to_mat(payload.to_vec());

        // clear terminal if size changes (to avoid artifacts)
        if size_changed {
            terminal.clear();
        }

        terminal.goto_topleft();

        terminal.write_frame(camera.get_frame());

        let stats = format!(
            "mem usage: {:.0}MB | pixels: {} ({}x{}) | fps: {:.0}",
            get_memory_usage(),
            camera.get_frame_num_pixels(),
            camera.get_frame_width(),
            camera.get_frame_height(),
            frame_count as f64 / begin.elapsed().as_secs_f64()
        );

        terminal.write_to_bottomright(&stats);

        // calculate fps based on moving frame rate every 3 seconds
        if begin.elapsed().as_secs() > 3 {
            frame_count = 0;
            begin = std::time::Instant::now();
        }
        frame_count += 1;
    }
}

// #[tokio::main]
// async fn main2() {
//     // add person to list of active users on firebase here

//     let users = firebase.at("users");
//     let result = users.set(&user).await.unwrap();
//     let data = result.data;
//     // parse data as json
//     let data: HashMap<String, String> = serde_json::from_str(&data).unwrap();
//     // get name attribute of json object
//     let my_id = data["name"].as_str();

//     // Home page loop

//     let contacts = firebase
//         .at("users")
//         .get::<HashMap<String, User>>()
//         .await
//         .unwrap();

//     let contacts = contacts
//         .iter()
//         .map(|(_k, v)| v.name.clone())
//         .collect::<Vec<String>>();

//     println!("{:?}", contacts);

//     println!(
//         "Hello, {}! This is the home page. If anyone calls you, you'll get a notification here. Who would you like to call?\nNames are case sensitive\n",
//         name
//     );
//     for (i, contact) in contacts.iter().enumerate() {
//         println!("{}: {}\n", i, contact);
//     }

//     let mut person_to_call = String::new();
//     let mut contacts = firebase
//         .at("users")
//         .get::<HashMap<String, User>>()
//         .await
//         .unwrap();

//     loop {
//         // poll for firebase changes here to see if we are receiving a call from anyone
//         // this is done by checking to see if the "sending_call" field is for any users is equal to our id.
//         // then we take that user's offer, and create our answer to it, and stick it in our object.
//         // the user will be listening for our answer, and once they take it in, we will be connected.

//         if event::poll(std::time::Duration::from_millis(50)).unwrap() {
//             contacts = firebase
//                 .at("users")
//                 .get::<HashMap<String, User>>()
//                 .await
//                 .unwrap();
//             println!("updated contacts");
//             let mut receiving_call = "null".to_string();

//             // check if anyone is sending a call to us
//             // TODO - change this to be realtime stream based. this way we can also update the list of contacts in realtime
//             for (k, v) in contacts.iter() {
//                 if k == my_id {
//                     receiving_call = v.receiving_call.clone();
//                     continue;
//                 }
//                 if v.sending_call == my_id && receiving_call == "null" {
//                     println!("receiving call from {}", v.name);
//                     // we are receiving a call
//                     // get the offer and create an answer
//                     let remote_offer = v.offer.clone();
//                     let (remote_sdp, remote_candidates) =
//                         serde_json::from_str(&remote_offer).unwrap();

//                     let mut remote_peer_connection =
//                         SimpleRemotePeerConnection::build(remote_sdp).await.unwrap();

//                     remote_peer_connection
//                         .add_ice_candidates(remote_candidates)
//                         .await
//                         .unwrap();

//                     // output answer and candidates for local peer
//                     let sdp = remote_peer_connection
//                         .get_local_description()
//                         .await
//                         .unwrap();
//                     let candidates = remote_peer_connection
//                         .collect_ice_candidates()
//                         .await
//                         .unwrap();

//                     // ... send the answer and the candidates back to Peer A via external signalling implementation ...
//                     let answer = (sdp, candidates);
//                     let answer = serde_json::to_string(&answer).unwrap();

//                     // update our user object with the answer
//                     let user = User {
//                         name: name.clone(),
//                         offer: "null".to_string(),
//                         answer,
//                         in_call: "null".to_string(),
//                         sending_call: "null".to_string(),
//                         receiving_call: k.to_string(),
//                     };
//                     let users = firebase.at("users");
//                     users.set(&user).await.unwrap();
//                 }
//             }
//         }

//         if event::poll(std::time::Duration::from_millis(50)).unwrap() {
//             if let event::Event::Key(event) = event::read().unwrap() {
//                 if event.code == event::KeyCode::Enter {
//                     let list_of_names = contacts
//                         .iter()
//                         .map(|(_k, v)| v.name.clone())
//                         .collect::<Vec<String>>();

//                     if list_of_names.contains(&&person_to_call.to_owned()) {
//                         break;
//                     }
//                     println!("that guy is not in the list of contacts!");
//                     person_to_call.clear();
//                 } else if event.code == event::KeyCode::Backspace {
//                     if person_to_call.len() > 0 {
//                         person_to_call.pop();
//                     }
//                 } else if let event::KeyCode::Char(c) = event.code {
//                     person_to_call.push(c);
//                 }
//             }
//         }

//         print!("{}", person_to_call);
//     }

//     // if we've gotten here it means we've selected a person to call and they are online
//     // we will now send them a call request

//     // get target person's id
//     let contacts = users.get::<HashMap<String, User>>().await.unwrap();

//     let target_id = contacts
//         .iter()
//         .find(|(_k, v)| v.name == person_to_call)
//         .unwrap()
//         .0;

//     // create offer here
//     let offer = local_peer_connection.get_local_description().await.unwrap();
//     let candidates = local_peer_connection
//         .collect_ice_candidates()
//         .await
//         .unwrap();

//     let signalling = serde_json::to_string(&(offer, candidates)).unwrap();

//     // update our user object with the sending_call field
//     let user = User {
//         name: name.clone(),
//         offer: signalling,
//         answer: "null".to_string(),
//         in_call: "null".to_string(),
//         sending_call: target_id.to_string(),
//         receiving_call: "null".to_string(),
//     };

//     let my_fb_location = users.at(my_id);
//     my_fb_location.update(&user).await.unwrap();

//     // now just wait for the answer to appear
//     loop {
//         if event::poll(std::time::Duration::from_millis(50)).unwrap() {
//             let users = firebase
//                 .at("users")
//                 .get::<HashMap<String, User>>()
//                 .await
//                 .unwrap();

//             // check if the person we are calling has accepted our call (they have created an answer)

//             if let Some(target) = users.get(target_id) {
//                 if target.answer != "null" {
//                     // we have received an answer
//                     // update our local peer connection with the answer
//                     let (remote_sdp, remote_candidates) =
//                         serde_json::from_str(&target.answer).unwrap();
//                     local_peer_connection
//                         .set_remote_description(remote_sdp)
//                         .await
//                         .unwrap();
//                     local_peer_connection
//                         .add_ice_candidates(remote_candidates)
//                         .await
//                         .unwrap();

//                     // update our user object with the in_call field
//                     let user = User {
//                         name: name.clone(),
//                         offer: "null".to_string(),
//                         answer: "null".to_string(),
//                         in_call: target_id.to_string(),
//                         sending_call: "null".to_string(),
//                         receiving_call: "null".to_string(),
//                     };
//                     let my_fb_location = firebase.at("users").at(my_id);
//                     my_fb_location.update(&user).await.unwrap();

//                     break;
//                 }
//             }
//         }
//     }

//     // if we've gotten here it means that the other person has created an answer and we've integrated it into our local peer connection
//     // so now we just have to await the connection to establish and data channelt o be made, then we can transmit data.

//     // -------todo

//     // In call loop
//     enable_raw_mode().unwrap();

//     let mut camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS).unwrap();
//     let mut microphone = Microphone::new();
//     let mut speaker = Speaker::new();

//     terminal.clear();
//     terminal.hide_cursor();

//     let mut frame_count = 0;
//     let mut begin = std::time::Instant::now();

//     loop {
//         // If q pressed, quit
//         if event::poll(std::time::Duration::from_millis(1)).unwrap() {
//             if let event::Event::Key(event) = event::read().unwrap() {
//                 if event.code == event::KeyCode::Char('q') {
//                     break;
//                 }
//             }
//         }

//         let (terminal_width, terminal_height, size_changed) = terminal.get_size();

//         assert!(camera.read_frame());
//         camera.resize_frame(terminal_width as f64, (terminal_height - 1) as f64, true);
//         camera.change_color_depth(24);

//         // clear terminal if size changes (to avoid artifacts)
//         if size_changed {
//             terminal.clear();
//         }

//         terminal.goto_topleft();

//         terminal.write_frame(camera.get_frame_mirrored());

//         let stats = format!(
//             "mem usage: {:.0}MB | pixels: {} ({}x{}) | fps: {:.0}",
//             get_memory_usage(),
//             camera.get_frame_num_pixels(),
//             camera.get_frame_width(),
//             camera.get_frame_height(),
//             frame_count as f64 / begin.elapsed().as_secs_f64()
//         );

//         terminal.write_to_bottomright(&stats);

//         // calculate fps based on moving frame rate every second
//         if begin.elapsed().as_secs() > 1 {
//             frame_count = 0;
//             begin = std::time::Instant::now();
//         }
//         frame_count += 1;
//     }
//     terminal.show_cursor();
//     disable_raw_mode().unwrap();
// }
