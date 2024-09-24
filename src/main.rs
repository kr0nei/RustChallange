use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::connect_async;
use tungstenite::Message;

#[tokio::main]
async fn main() {

    if let Some(message) = get_data_from_binance().await {
        println!("Received message: {:?}", message);
    }

    if let Some(message) = get_data_from_bitstamp().await {
        println!("Received message: {:?}", message);
    }




}

async fn get_data_from_bitstamp() -> Option<String> {
    let bitstamp_url = "wss://ws.bitstamp.net";
    let (bitstamp_ws_stream, _) = connect_async(bitstamp_url).await.expect("Failed to connect");
    println!("Connected to bitstamp");

    let (mut bitstamp_write, mut bitstamp_read) = bitstamp_ws_stream.split();
    let bitstamp_subscribe_json = json!({
        "event": "bts:subscribe",
        "data": {
            "channel": "order_book_ethbtc"
        }
    });
    let bitstamp_subscribe = Message::Text(bitstamp_subscribe_json.to_string());
    bitstamp_write.send(bitstamp_subscribe).await.expect("Failed to subscribe");

    if let Some(msg) = bitstamp_read.next().await {
        match msg {
            Ok(msg) => {
                if let Message::Text(text) = msg {
                    let json_message: Value = serde_json::from_str(&text).expect("Failed to parse JSON");

                    if let Some(event) = json_message.get("event") {
                        if event == "bts:subscription_succeeded" {
                            if let Some(message) = bitstamp_read.next().await {
                                let data = message.expect("Failed to receive message");
                                if let Message::Text(dat) = data {
                                    Some(dat)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }else {
                    None
                }
            }
            Err(_e) => {
                None
            }
        }
    } else {
        None
    }
}

async fn get_data_from_binance() -> Option<String> {
    let binance_url = "wss://stream.binance.com:9443/ws/ethbtc@depth20@100ms";
    let (binance_ws_stream, _) = connect_async(binance_url).await.expect("Failed to connect");
    println!("Connected to binance");

    let (mut binance_write, mut binance_read) = binance_ws_stream.split();
    let binance_subscribe_json = json!({
        "method": "SUBSCRIBE",
        "params": [
            "ethbtc@aggTrade",
            "ethbtc@depth"
        ],
        "id": 2
    });
    let binance_subscribe = Message::Text(binance_subscribe_json.to_string());

    binance_write.send(binance_subscribe).await.expect("Failed to subscribe");

    if let Some(message) = binance_read.next().await {
        let message = message.expect("Failed to receive message");
        if let Message::Text(dat) = message {
            Some(dat)
        } else {
            None
        }
        //time::delay_for(time::Duration::from_millis(2000));
    } else {
        None
    }
}