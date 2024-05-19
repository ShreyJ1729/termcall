use std::io::Write;

use just_webrtc::{
    types::{ICECandidate, SessionDescription},
    DataChannelExt, PeerConnectionExt, SimpleLocalPeerConnection,
};

#[tokio::main]
async fn main() {
    // create simple local peer connection with unordered data channel
    let mut local_peer_connection = SimpleLocalPeerConnection::build(false).await.unwrap();

    // output offer and candidates for remote peer
    let offer = local_peer_connection.get_local_description().await.unwrap();
    let candidates = local_peer_connection
        .collect_ice_candidates()
        .await
        .unwrap();

    // ... send the offer and the candidates to Peer B via external signalling implementation ...
    let signalling = (offer, candidates);

    // write it to file
    let mut file = std::fs::File::create("signalling.txt").unwrap();
    let data = bincode::serialize(&signalling).unwrap();
    // data sent to firebase rtdb by turning vec of bytes into string that is b1-b2-b3-...-bn, then read back in on the other side
    file.write_all(&data).unwrap();

    println!("Generated signaling offer and wrote to file. Press enter once answer.txt has been created.");

    // pause until user presses enter
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    // then read in the file answer.txt
    let file = std::fs::File::open("answer.txt").unwrap();
    let signalling: (SessionDescription, Vec<ICECandidate>) =
        bincode::deserialize_from(file).unwrap();

    // ... receive the answer and candidates from Peer B via external signalling implementation ...
    let (answer, candidates) = signalling;

    // update local peer from received answer and candidates
    local_peer_connection
        .set_remote_description(answer)
        .await
        .unwrap();
    local_peer_connection
        .add_ice_candidates(candidates)
        .await
        .unwrap();

    println!("Received signaling answer and candidates. Waiting for connection to complete.");

    // local signalling is complete! we can now wait for a complete connection
    local_peer_connection.wait_peer_connected().await;

    println!("Peer connection complete!");

    // receive data channel from local peer
    let mut local_channel = local_peer_connection.receive_channel().await.unwrap();

    // wait for data channels to be ready
    local_channel.wait_ready().await;

    println!("Data channel ready!");

    // recv data from remote (answerer)
    loop {
        // send data to remote (answerer)
        local_channel
            .send(&bytes::Bytes::from("hello remote!"))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let recv = local_channel.receive().await.unwrap();

        println!("Received: {:?}", recv);

        // pause for 1 second
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
