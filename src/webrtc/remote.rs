use std::io::Write;

use just_webrtc::{
    types::{ICECandidate, SessionDescription},
    DataChannelExt, PeerConnectionExt, SimpleRemotePeerConnection,
};

#[tokio::main]
async fn main() {
    // ... receive the offer and the candidates from Peer A via external signalling implementation ...
    // read from file
    let file = std::fs::File::open("signalling.txt").expect("something failed");
    let signalling: (SessionDescription, Vec<ICECandidate>) =
        bincode::deserialize_from(file).expect("something failed");
    let (offer, candidates) = signalling;

    // create simple remote peer connection from received offer and candidates
    let mut remote_peer_connection = SimpleRemotePeerConnection::build(offer)
        .await
        .expect("something failed");
    remote_peer_connection
        .add_ice_candidates(candidates)
        .await
        .expect("something failed");
    // output answer and candidates for local peer
    let answer = remote_peer_connection
        .get_local_description()
        .await
        .expect("something failed");
    let candidates = remote_peer_connection
        .collect_ice_candidates()
        .await
        .expect("something failed");

    // ... send the answer and the candidates back to Peer A via external signalling implementation ...
    let _signalling = (answer, candidates);

    // write it to file
    let mut file = std::fs::File::create("answer.txt").expect("something failed");
    let data = bincode::serialize(&_signalling).expect("something failed");
    file.write_all(&data).expect("something failed");

    // remote signalling is complete! we can now wait for a complete connection
    remote_peer_connection.wait_peer_connected().await;

    println!("Peer connection complete!");

    // receive data channel from local and remote peers
    let mut remote_channel = remote_peer_connection
        .receive_channel()
        .await
        .expect("Failed to receive data channel!");
    // wait for data channels to be ready
    remote_channel.wait_ready().await;

    println!("Data channel ready!");

    // send/recv data from local (offerer) to remote (answerer)
    loop {
        let recv = remote_channel.receive().await.expect("something failed");
        println!("Received: {:?}", recv);

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        remote_channel
            .send(&bytes::Bytes::from("hello local!"))
            .await
            .expect("Failed to send data to local peer!");

        // pause for 1 second
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
