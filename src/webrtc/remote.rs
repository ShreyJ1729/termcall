use anyhow::Result;
use just_webrtc::{
    types::{ICECandidate, SessionDescription},
    DataChannelExt, PeerConnectionExt, SimpleRemotePeerConnection,
};

async fn run_remote_peer(offer: SessionDescription, candidates: Vec<ICECandidate>) -> Result<()> {
    // ... receive the offer and the candidates from Peer A via external signalling implementation ...

    // create simple remote peer connection from received offer and candidates
    let mut remote_peer_connection = SimpleRemotePeerConnection::build(offer).await?;
    remote_peer_connection
        .add_ice_candidates(candidates)
        .await?;
    // output answer and candidates for local peer
    let answer = remote_peer_connection
        .get_local_description()
        .await
        .unwrap();
    let candidates = remote_peer_connection.collect_ice_candidates().await?;

    // ... send the answer and the candidates back to Peer A via external signalling implementation ...
    let _signalling = (answer, candidates);

    // remote signalling is complete! we can now wait for a complete connection
    remote_peer_connection.wait_peer_connected().await;

    // receive data channel from local and remote peers
    let mut remote_channel = remote_peer_connection.receive_channel().await.unwrap();
    // wait for data channels to be ready
    remote_channel.wait_ready().await;

    // send/recv data from local (offerer) to remote (answerer)
    let recv = remote_channel.receive().await.unwrap();
    assert_eq!(&recv, "hello remote!");
    // send/recv data from remote (answerer) to local (offerer)
    remote_channel
        .send(&bytes::Bytes::from("hello local!"))
        .await?;

    Ok(())
}
