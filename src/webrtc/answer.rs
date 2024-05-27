use anyhow::Result;
use std::{
    sync::{atomic, Arc, Mutex},
    time::Duration,
};
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder,
    },
    data_channel::{data_channel_message::DataChannelMessage, RTCDataChannel},
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
            // delete answer and candidates files
            std::fs::remove_file("answer").unwrap();
            std::fs::remove_file("answer_candidates").unwrap();
        }
        Box::pin(async {})
    }));

    // Data channel init
    peer_connection.on_data_channel(Box::new(|d| {
        println!("New DataChannel: {} {}", d.label(), d.id());
        let d2 = d.clone();

        d.on_open(Box::new(move || {
            println!("Data channel is open");
            Box::pin(async move {
                let mut result = Result::<usize>::Ok(0);
                while result.is_ok() {
                    let timeout = tokio::time::sleep(Duration::from_secs(5));
                    tokio::pin!(timeout);

                    tokio::select! {
                        _ = timeout.as_mut() =>{
                            let message = "Hello from answer";
                            println!("Sending '{message}'");
                            result = d2.send_text(message).await.map_err(Into::into);
                        }
                    };
                }
            })
        }));

        d.on_message(Box::new(move |msg: DataChannelMessage| {
            println!("Message from DataChannel: {:?}", msg);
            Box::pin(async move {})
        }));

        Box::pin(async move {})
    }));

    // receive offer
    while !std::fs::metadata("offer").is_ok() || !std::fs::metadata("offer_candidates").is_ok() {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let offer_str = std::fs::read_to_string("offer")?;
    let candidates_str = std::fs::read_to_string("offer_candidates")?;

    let offer = serde_json::from_str::<RTCSessionDescription>(&offer_str)?;
    let candidates = serde_json::from_str::<Vec<RTCIceCandidate>>(&candidates_str)?;

    peer_connection.set_remote_description(offer).await?;
    for c in candidates {
        peer_connection
            .add_ice_candidate(RTCIceCandidateInit {
                candidate: c.to_json().unwrap().candidate,
                ..Default::default()
            })
            .await?;
    }

    // Create an answer
    let answer = peer_connection.create_answer(None).await?;
    let answer_str = serde_json::to_string(&answer)?;

    // Sets the LocalDescription, and starts our UDP listeners
    // Note: this will start the gathering of ICE candidates
    peer_connection.set_local_description(answer).await?;

    while !all_candidates_built.load(atomic::Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let candidates_str = serde_json::to_string(&*candidate_list.lock().unwrap())?;

    // write answer and candidates to file
    std::fs::write("answer", answer_str)?;
    std::fs::write("answer_candidates", candidates_str)?;

    // Block forever
    tokio::signal::ctrl_c().await?;

    Ok(())
}
