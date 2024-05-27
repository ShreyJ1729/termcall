use std::sync::{
    atomic::{self, AtomicBool},
    Arc, Mutex,
};

use super::config::get_default_config;
use anyhow::Result;
use webrtc::{
    api::APIBuilder,
    data_channel::{data_channel_message::DataChannelMessage, RTCDataChannel},
    ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
    peer_connection::{
        self, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
};

pub struct RTCOffererConnection {
    pub peer_connection: Arc<Mutex<RTCPeerConnection>>,
    pub candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
    pub all_candidates_gathered: Arc<AtomicBool>,
    pub data_channels: Vec<Arc<RTCDataChannel>>,
}

impl RTCOffererConnection {
    pub async fn new() -> Result<Self> {
        let api = APIBuilder::new().build();
        let peer_connection = api.new_peer_connection(get_default_config()).await?;
        let peer_connection = Arc::new(Mutex::new(peer_connection));
        let offerer_connection = Self {
            peer_connection,
            candidates: Arc::new(Mutex::new(Vec::new())),
            all_candidates_gathered: Arc::new(AtomicBool::new(false)),
            data_channels: Vec::new(),
        };

        // Peer connection event handlers
        offerer_connection.register_pc_on_ice_candidates();
        offerer_connection.register_pc_connection_state_change();

        // Data channel event handlers
        offerer_connection.register_dc_on_open();
        offerer_connection.register_dc_on_message();

        Ok(offerer_connection)
    }

    pub async fn create_offer(&self) -> Result<RTCSessionDescription> {
        let pc = self.peer_connection.lock().unwrap();
        let offer = pc.create_offer(None).await?;
        Ok(offer)
    }

    pub async fn set_local_description(&self, offer: RTCSessionDescription) -> Result<()> {
        let pc = self.peer_connection.lock().unwrap();
        pc.set_local_description(offer).await?;
        Ok(())
    }

    pub async fn set_remote_description(&self, answer: RTCSessionDescription) -> Result<()> {
        let pc = self.peer_connection.lock().unwrap();
        pc.set_remote_description(answer).await?;
        Ok(())
    }

    pub async fn add_ice_candidates(&self, candidates: Vec<RTCIceCandidate>) -> Result<()> {
        let pc = self.peer_connection.lock().unwrap();
        let candidates = self.candidates.lock().unwrap();
        for candidate in candidates.iter() {
            let candidate_init = RTCIceCandidateInit {
                candidate: candidate.to_json()?.candidate,
                ..Default::default()
            };
            pc.add_ice_candidate(candidate_init).await?;
        }
        Ok(())
    }

    pub async fn create_data_channel(&mut self, label: &str) -> Result<Arc<RTCDataChannel>> {
        let pc = self.peer_connection.lock().unwrap();
        let dc = pc.create_data_channel(label, None).await?;
        self.data_channels.push(dc.clone());
        Ok(dc)
    }

    pub fn register_dc_on_open(&self) {
        for dc in self.data_channels.iter() {
            let dc = dc.clone();
            let dc_label = dc.label().to_owned();
            dc.on_open(Box::new(move || {
                Box::pin(async move {
                    println!("Data channel {} is now open", dc_label);
                })
            }));
        }
    }

    pub fn register_dc_on_message(&self) {
        for dc in self.data_channels.iter() {
            let dc = dc.clone();
            let dc_label = dc.label().to_owned();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let msg = msg.clone();
                let dc_label = dc_label.clone();
                Box::pin(async move {
                    if msg.is_string {
                        let msg = String::from_utf8_lossy(&msg.data);
                        println!("{} got message: {:?}", dc_label, msg);
                        return;
                    }
                })
            }));
        }
    }

    pub fn register_pc_on_ice_candidates(&self) {
        let pc = self.peer_connection.lock().unwrap();
        let cs = self.candidates.clone();
        let acg = self.all_candidates_gathered.clone();
        pc.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
            let cs = cs.clone();
            let acg = acg.clone();
            Box::pin(async move {
                let mut cs = cs.lock().unwrap();
                if let Some(c) = c {
                    cs.push(c);
                } else {
                    acg.store(true, atomic::Ordering::SeqCst);
                }
            })
        }));
    }

    pub fn register_pc_connection_state_change(&self) {
        let pc = self.peer_connection.lock().unwrap();
        pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            println!("Peer Connection State has changed: {state}");
            if state == RTCPeerConnectionState::Disconnected {
                // todo: Graceful shutdown
            }
            Box::pin(async move {})
        }));
    }
}
