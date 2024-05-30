use std::{
    fmt::Debug,
    sync::{
        atomic::{self, AtomicBool},
        Arc, Mutex,
    },
};

use anyhow::Result;
use simple_log::{error, info, warn};
use tokio::sync::mpsc;
use webrtc::{
    api::APIBuilder,
    data_channel::{
        data_channel_message::DataChannelMessage, data_channel_state::RTCDataChannelState,
        RTCDataChannel,
    },
    ice_transport::{
        ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
        ice_server::RTCIceServer,
    },
    peer_connection::{
        configuration::RTCConfiguration, math_rand_alpha,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
};

pub struct PeerConnection {
    pub id: String,
    pub rtc_pc: Arc<Mutex<RTCPeerConnection>>,
    pub candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
    pub gathering_done: Arc<AtomicBool>,
    pub state: Arc<Mutex<RTCPeerConnectionState>>,
    pub data_channels: Arc<Mutex<Vec<Arc<RTCDataChannel>>>>,
    pub on_message_tx: Arc<Mutex<mpsc::Sender<DataChannelMessage>>>,
    pub on_message_rx: Arc<Mutex<mpsc::Receiver<DataChannelMessage>>>,
    pub on_close_tx: Arc<Mutex<mpsc::Sender<()>>>,
    pub on_close_rx: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl PeerConnection {
    pub async fn new() -> Result<Self> {
        let api = APIBuilder::default().build();
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let peer_connection = api.new_peer_connection(config).await?;
        let peer_connection = Arc::new(Mutex::new(peer_connection));

        let (on_message_tx, on_message_rx) = mpsc::channel(1);
        let (on_close_tx, on_close_rx) = mpsc::channel(1);
        let on_message_tx = Arc::new(Mutex::new(on_message_tx));
        let on_message_rx = Arc::new(Mutex::new(on_message_rx));
        let on_close_tx = Arc::new(Mutex::new(on_close_tx));
        let on_close_rx = Arc::new(Mutex::new(on_close_rx));

        let id = math_rand_alpha(5);

        let mut peer_connection = Self {
            id,
            rtc_pc: peer_connection,
            candidates: Arc::new(Mutex::new(Vec::new())),
            gathering_done: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(RTCPeerConnectionState::New)),
            data_channels: Arc::new(Mutex::new(Vec::new())),
            on_message_tx,
            on_message_rx,
            on_close_tx,
            on_close_rx,
        };

        // Peer connection event handlers
        peer_connection.register_pc_on_ice_candidates();
        peer_connection.register_pc_connection_state_change();
        peer_connection.register_pc_on_data_channel();

        peer_connection
            .create_data_channel(format!("{}-video-send", peer_connection.id).as_str())
            .await?;

        peer_connection
            .create_data_channel(format!("{}-audio-send", peer_connection.id).as_str())
            .await?;

        peer_connection.register_dcs_on_open();
        peer_connection.register_dcs_on_message();

        Ok(peer_connection)
    }

    pub async fn create_offer(&self) -> Result<RTCSessionDescription> {
        let pc = self.rtc_pc.lock().unwrap();
        let local_sd = pc.create_offer(None).await?;
        Ok(local_sd)
    }

    pub async fn create_answer(&self) -> Result<RTCSessionDescription> {
        let pc = self.rtc_pc.lock().unwrap();
        let local_sd = pc.create_answer(None).await?;
        Ok(local_sd)
    }

    pub async fn set_local_description(&self, local_sd: RTCSessionDescription) -> Result<()> {
        let pc = self.rtc_pc.lock().unwrap();
        pc.set_local_description(local_sd).await?;
        Ok(())
    }

    pub async fn set_remote_description(&self, remote_sd: RTCSessionDescription) -> Result<()> {
        let pc = self.rtc_pc.lock().unwrap();
        pc.set_remote_description(remote_sd).await?;
        Ok(())
    }

    pub async fn get_ice_candidates(&self) -> Vec<RTCIceCandidate> {
        let ice_cs = self.candidates.clone();
        let ice_cs = ice_cs.lock().unwrap();
        ice_cs.clone()
    }

    pub async fn add_remote_ice_candidates(&self, candidates: Vec<RTCIceCandidate>) -> Result<()> {
        let pc = self.rtc_pc.lock().unwrap();
        for candidate in candidates.iter() {
            let candidate_init = RTCIceCandidateInit {
                candidate: candidate.to_json()?.candidate,
                ..Default::default()
            };
            pc.add_ice_candidate(candidate_init).await?;
        }
        Ok(())
    }

    pub async fn create_data_channel(&mut self, label: &str) -> Result<(), webrtc::Error> {
        let pc = self.rtc_pc.lock().unwrap();
        let dcs = self.data_channels.clone();
        let mut dcs = dcs.lock().unwrap();

        let dc = match pc.create_data_channel(label, None).await {
            Ok(dc) => dc,
            Err(e) => {
                error!("Failed to create data channel: {}", e);
                return Err(e);
            }
        };
        dcs.push(dc.clone());
        Ok(())
    }

    pub fn register_dcs_on_open(&self) {
        let dcs = self.data_channels.clone();
        let dcs = dcs.lock().unwrap();
        for dc in dcs.iter() {
            let dc = dc.clone();
            let dc_label = dc.label().to_owned();
            dc.on_open(Box::new(move || {
                Box::pin(async move {
                    info!("Data channel {} is now open", dc_label);
                })
            }));
        }
    }

    pub fn register_dcs_on_message(&self) {
        let dcs = self.data_channels.clone();
        let dcs = dcs.lock().unwrap();
        for dc in dcs.iter() {
            let dc = dc.clone();
            let dc_label = dc.label().to_owned();
            let on_message_tx = self.on_message_tx.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let msg = msg.clone();
                info!("Data channel {} received message: {:?}", dc_label, msg);
                if dc_label.contains("audio") {
                    return Box::pin(async move {});
                }
                let on_message_tx = on_message_tx.lock().unwrap();
                match on_message_tx.try_send(msg) {
                    Ok(_) => {
                        info!("Message successfully sent to on_message_tx");
                    }
                    Err(e) => {
                        warn!("Failed to send message - rx closed or buffer full: {}", e)
                    }
                }
                Box::pin(async move {})
            }));
        }
    }

    pub fn register_pc_on_ice_candidates(&self) {
        let pc = self.rtc_pc.lock().unwrap();
        let ice_cs = self.candidates.clone();
        let acg = self.gathering_done.clone();
        pc.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
            info!("New ICE Candidate: {:?}", c);
            let ice_cs = ice_cs.clone();
            let acg = acg.clone();
            Box::pin(async move {
                let mut ice_cs = ice_cs.lock().unwrap();
                if let Some(c) = c {
                    ice_cs.push(c);
                } else {
                    acg.store(true, atomic::Ordering::SeqCst);
                    info!("All ICE Candidates have been gathered");
                }
            })
        }));
    }

    pub fn register_pc_connection_state_change(&self) {
        let pc = self.rtc_pc.lock().unwrap();
        let pc_state = self.state.clone();
        let on_close_tx = self.on_close_tx.clone();
        pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            info!("Peer Connection State has changed: {state}");
            let mut pc_state = pc_state.lock().unwrap();
            *pc_state = state;

            if state == RTCPeerConnectionState::Disconnected {
                on_close_tx.lock().unwrap().try_send(()).unwrap()
            }
            Box::pin(async move {})
        }));
    }

    pub fn register_pc_on_data_channel(&self) {
        let pc = self.rtc_pc.lock().unwrap();
        let dcs = self.data_channels.clone();
        let on_message_tx = self.on_message_tx.clone();
        pc.on_data_channel(Box::new(move |d| {
            info!("New DataChannel Received: {} {}", d.label(), d.id());
            let mut dcs = dcs.lock().unwrap();
            let dc = d.clone();
            dcs.push(dc.clone());

            let dc_label = dc.label().to_owned();
            let dc_label2 = dc_label.clone();
            let on_message_tx = on_message_tx.clone();

            dc.on_open(Box::new(move || {
                info!("Data channel {} is now open", dc_label);
                Box::pin(async move {})
            }));

            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let on_message_tx = on_message_tx.lock().unwrap();
                if dc_label2.contains("audio") {
                    return Box::pin(async move {});
                }
                info!("Data channel {} received message: {:?}", dc_label2, msg);
                match on_message_tx.try_send(msg) {
                    Ok(_) => {
                        info!("Message successfully sent to on_message_tx");
                    }
                    Err(e) => {
                        warn!("Failed to send message - rx closed or buffer full: {}", e);
                    }
                }
                Box::pin(async move {})
            }));

            Box::pin(async move {})
        }));
    }

    pub async fn wait_peer_connected(&self) {
        let pc_state = self.state.clone();
        while *pc_state.lock().unwrap() != RTCPeerConnectionState::Connected {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn wait_ice_candidates_gathered(&self) {
        while !self.gathering_done.load(atomic::Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn wait_data_channels_open(&self) {
        let dcs = self.data_channels.clone();
        let dcs = dcs.lock().unwrap();

        for dc in dcs.iter() {
            while dc.ready_state() != RTCDataChannelState::Open {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    pub async fn close(&self) {
        let pc = self.rtc_pc.lock().unwrap();
        pc.close().await.unwrap();
    }

    pub async fn get_data_channel(&self, label: &str) -> Option<Arc<RTCDataChannel>> {
        let dcs = self.data_channels.clone();
        let dcs = dcs.lock().unwrap();
        for dc in dcs.iter() {
            if dc.label() == label {
                return Some(dc.clone());
            }
        }

        error!("Data channel {} not found", label);
        None
    }
}
