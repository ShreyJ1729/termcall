use anyhow::Result;
use just_webrtc::{
    types::{ICECandidate, SessionDescription},
    DataChannelExt, PeerConnectionExt, SimpleLocalPeerConnection,
};

async fn run_local_peer() -> Result<()> {
    // create simple local peer connection with unordered data channel
    let mut local_peer_connection = SimpleLocalPeerConnection::build(false).await?;

    // output offer and candidates for remote peer
    let offer = local_peer_connection.get_local_description().await.unwrap();
    let candidates = local_peer_connection.collect_ice_candidates().await?;

    // ... send the offer and the candidates to Peer B via external signalling implementation ...
    let signalling = (offer, candidates);

    // ... receive the answer and candidates from Peer B via external signalling implementation ...
    let (answer, candidates) = signalling;

    // update local peer from received answer and candidates
    local_peer_connection.set_remote_description(answer).await?;
    local_peer_connection.add_ice_candidates(candidates).await?;

    // local signalling is complete! we can now wait for a complete connection
    local_peer_connection.wait_peer_connected().await;

    // receive data channel from local peer
    let mut local_channel = local_peer_connection.receive_channel().await.unwrap();
    // wait for data channels to be ready
    local_channel.wait_ready().await;

    // send data to remote (answerer)
    local_channel
        .send(&bytes::Bytes::from("hello remote!"))
        .await?;
    // recv data from remote (answerer)
    let recv = local_channel.receive().await.unwrap();
    assert_eq!(&recv, "hello local!");
}
