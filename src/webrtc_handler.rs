use std::sync::Arc;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

pub struct WebRTC_Handler {
    pub peer_connection: RTCPeerConnection,
    pub video_channel: Arc<RTCDataChannel>,
    pub audio_channel: Arc<RTCDataChannel>,
}
impl WebRTC_Handler {
    pub async fn new() -> WebRTC_Handler {
        // Setup media engine to use default codecs
        let mut m = MediaEngine::default();
        m.register_default_codecs().unwrap();

        // Setup Interceptor and API
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m).unwrap();
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // Create config including ice servers
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let peer_connection = api.new_peer_connection(config).await.unwrap();
        let video_channel = peer_connection
            .create_data_channel("video", None)
            .await
            .unwrap();

        let audio_channel = peer_connection
            .create_data_channel("video", None)
            .await
            .unwrap();

        register_dc_event_listeners(video_channel.clone(), audio_channel.clone());

        WebRTC_Handler {
            peer_connection,
            video_channel,
            audio_channel,
        }
    }

    pub async fn create_offer(&self) -> Result<RTCSessionDescription, webrtc::Error> {
        self.peer_connection.create_offer(None).await
    }

    pub async fn set_local_description(
        &self,
        session_description: String,
    ) -> Result<(), webrtc::Error> {
        self.peer_connection
            .set_local_description(RTCSessionDescription::offer(session_description).unwrap())
            .await
    }

    pub async fn set_remote_description(
        &self,
        session_description: String,
    ) -> Result<(), webrtc::Error> {
        self.peer_connection
            .set_remote_description(RTCSessionDescription::offer(session_description).unwrap())
            .await
    }
}

pub fn register_dc_event_listeners(
    video_channel: Arc<RTCDataChannel>,
    audio_channel: Arc<RTCDataChannel>,
) {
    video_channel.on_open(Box::new(move || {
        println!("Data channel 'video' is open");
        Box::pin(async {})
    }));

    video_channel.on_message(Box::new(move |msg: DataChannelMessage| {
        println!("Data channel 'video' received message: {:?}", msg);
        Box::pin(async {})
    }));

    video_channel.on_close(Box::new(|| {
        println!("Data channel 'video' is closed");
        Box::pin(async {})
    }));

    audio_channel.on_open(Box::new(move || {
        println!("Data channel 'audio' is open");
        Box::pin(async {})
    }));

    audio_channel.on_message(Box::new(move |msg: DataChannelMessage| {
        println!("Data channel 'audio' received message: {:?}", msg);
        Box::pin(async {})
    }));

    audio_channel.on_close(Box::new(|| {
        println!("Data channel 'audio' is closed");
        Box::pin(async {})
    }));
}
