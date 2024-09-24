use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::connect_async;
use tungstenite::Message;

#[tokio::main]
async fn main() {
    let url = "wss://stream.binance.com:9443/ws/ethbtc@depth20@100ms";
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("Connected to binance");

    let (mut write, mut read) = ws_stream.split();
    let subscribe_json = json!({
        "method": "SUBSCRIBE",
        "params": [
            "ethbtc@aggTrade",
            "ethbtc@depth"
        ],
        "id": 2
    });
    let subscribe = Message::Text(subscribe_json.to_string());

    write.send(subscribe).await.expect("Failed to subscribe");

    if let Some(message) = read.next().await {
        let message = message.expect("Failed to receive message");
        println!("Received message: {:?}", message);
    }
}