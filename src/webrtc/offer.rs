use anyhow::Result;
use std::sync::{atomic, Arc, Mutex};
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder,
    },
    data_channel::data_channel_message::DataChannelMessage,
    ice_transport::{
        ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
        ice_server::RTCIceServer,
    },
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    let candidate_list: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(Vec::new()));

    // Configuration stuff
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut m)?;
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Peer connection init
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    let pc = peer_connection.clone();
    let cand_list = candidate_list.clone();
    let all_candidates_built = Arc::new(atomic::AtomicBool::new(false));
    let acb = Arc::clone(&all_candidates_built);
    peer_connection.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
        let cand_list = Arc::clone(&cand_list);
        let acb2 = Arc::clone(&acb);

        Box::pin(async move {
            let mut cs = cand_list.lock().unwrap();
            if let Some(c) = c {
                cs.push(c);
            } else {
                acb2.store(true, atomic::Ordering::SeqCst);
            }
        })
    }));

    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");
        if s == RTCPeerConnectionState::Connected {
            // delete offer and candidates files
            std::fs::remove_file("offer").unwrap();
            std::fs::remove_file("offer_candidates").unwrap();
        }
        Box::pin(async {})
    }));

    // Data channel init
    let data_channel = peer_connection.create_data_channel("data", None).await?;

    data_channel.on_open(Box::new(move || {
        println!("Data channel is open");
        Box::pin(async {})
    }));

    data_channel.on_message(Box::new(move |msg: DataChannelMessage| {
        println!("Data channel message: {:?}", msg);
        Box::pin(async {})
    }));

    // Create an offer to send to a peer
    let offer = peer_connection.create_offer(None).await?;
    let offer_str = serde_json::to_string(&offer)?;
    let candidates_str = serde_json::to_string(&*candidate_list.lock().unwrap())?;

    // Sets the LocalDescription, and starts our UDP listeners
    // Note: this will start the gathering of ICE candidates
    peer_connection.set_local_description(offer).await?;

    while !all_candidates_built.load(atomic::Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // Once all candidates gathered, write both offer and candidates to file
    std::fs::write("offer", offer_str)?;
    std::fs::write("offer_candidates", candidates_str)?;

    while !std::fs::metadata("answer").is_ok() || !std::fs::metadata("answer_candidates").is_ok() {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // receive answer
    let answer_str = std::fs::read_to_string("answer")?;
    let candidates_str = std::fs::read_to_string("answer_candidates")?;
    let answer = serde_json::from_str::<RTCSessionDescription>(&answer_str)?;
    let candidates = serde_json::from_str::<Vec<RTCIceCandidate>>(&candidates_str)?;

    peer_connection.set_remote_description(answer).await?;
    for c in candidates {
        peer_connection
            .add_ice_candidate(RTCIceCandidateInit {
                candidate: c.to_json().unwrap().candidate,
                ..Default::default()
            })
            .await?;
    }

    // block forever
    while tokio::signal::ctrl_c().await.is_err() {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        data_channel.send_text("Hello, World!").await?;
    }

    Ok(())
}
