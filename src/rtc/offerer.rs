use std::sync::{
    atomic::{self, AtomicBool},
    Arc, Mutex,
};

use super::config::get_default_config;
use anyhow::Result;
use tokio::sync::mpsc;
use webrtc::{
    api::APIBuilder,
    data_channel::{
        data_channel_message::DataChannelMessage, data_channel_state::RTCDataChannelState,
        RTCDataChannel,
    },
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
    pub peer_connected: Arc<AtomicBool>,
    pub data_channels: Vec<Arc<RTCDataChannel>>,
    pub on_message_tx: Arc<Mutex<mpsc::Sender<DataChannelMessage>>>,
    pub on_message_rx: Arc<Mutex<mpsc::Receiver<DataChannelMessage>>>,
}

impl RTCOffererConnection {
    pub async fn new() -> Result<Self> {
        let api = APIBuilder::default().build();
        let peer_connection = api.new_peer_connection(get_default_config()).await?;
        let peer_connection = Arc::new(Mutex::new(peer_connection));

        let (on_message_tx, on_message_rx) = mpsc::channel(100);
        let on_message_tx = Arc::new(Mutex::new(on_message_tx));
        let on_message_rx = Arc::new(Mutex::new(on_message_rx));

        let mut offerer_connection = Self {
            peer_connection,
            candidates: Arc::new(Mutex::new(Vec::new())),
            all_candidates_gathered: Arc::new(AtomicBool::new(false)),
            peer_connected: Arc::new(AtomicBool::new(false)),
            data_channels: Vec::new(),
            on_message_tx,
            on_message_rx,
        };

        // Peer connection event handlers
        offerer_connection.register_pc_on_ice_candidates();
        offerer_connection.register_pc_connection_state_change();

        offerer_connection.create_data_channel("video").await?;

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
        for candidate in candidates.iter() {
            let candidate_init = RTCIceCandidateInit {
                candidate: candidate.to_json()?.candidate,
                ..Default::default()
            };
            pc.add_ice_candidate(candidate_init).await?;
            println!(
                "Added remote ICE candidate: {}",
                candidate.to_json()?.candidate
            );
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
            let on_message_tx = self.on_message_tx.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let msg = msg.clone();
                let on_message_tx = on_message_tx.lock().unwrap();
                on_message_tx.try_send(msg).unwrap();
                Box::pin(async move {})
            }));
        }
    }

    pub fn register_pc_on_ice_candidates(&self) {
        let pc = self.peer_connection.lock().unwrap();
        let cs = self.candidates.clone();
        let acg = self.all_candidates_gathered.clone();
        pc.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
            println!("New ICE Candidate: {:?}", c);
            let cs = cs.clone();
            let acg = acg.clone();
            Box::pin(async move {
                let mut cs = cs.lock().unwrap();
                if let Some(c) = c {
                    cs.push(c);
                } else {
                    acg.store(true, atomic::Ordering::SeqCst);
                    println!("All ICE Candidates have been gathered");
                }
            })
        }));
    }

    pub fn register_pc_connection_state_change(&self) {
        let pc = self.peer_connection.lock().unwrap();
        let peer_connected = self.peer_connected.clone();
        pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            println!("Peer Connection State has changed: {state}");
            if state == RTCPeerConnectionState::Connected {
                peer_connected.store(true, atomic::Ordering::SeqCst);
            }
            if state == RTCPeerConnectionState::Disconnected {
                // todo: Graceful shutdown
            }
            Box::pin(async move {})
        }));
    }

    pub async fn wait_peer_connected(&self) {
        while !self.peer_connected.load(atomic::Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        println!("Peer connected");
    }

    pub async fn wait_data_channels_open(&self) {
        for dc in self.data_channels.iter() {
            while dc.ready_state() != RTCDataChannelState::Open {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    pub async fn get_data_channel(&self, index: usize) -> Option<Arc<RTCDataChannel>> {
        if index >= self.data_channels.len() {
            return None;
        }
        Some(self.data_channels[index].clone())
    }
}
