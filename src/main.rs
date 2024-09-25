use std::cmp::Ordering;
use std::string::String;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tungstenite::Message;
use clap::Parser;
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;

#[derive(Clone)]
struct Order {
    price: f64,
    quantity: f64,
    exchange: String,
}

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    currency_pair: String
}


#[tokio::main]
async fn main() {
    let args = Args::parse();
    let currency_pair: String;

    if let pair = args.currency_pair {
        currency_pair = pair;
    } else {
        currency_pair = "ethbtc".to_string();
    }

    let bitstamp_url = "wss://ws.bitstamp.net";
    let (bitstamp_ws_stream, _) = connect_async(bitstamp_url).await.expect("Failed to connect");
    println!("Connected to bitstamp");

    let (mut bitstamp_write, mut bitstamp_read) = bitstamp_ws_stream.split();

    bitstamp_subscribe(currency_pair.to_string(), &mut bitstamp_write).await;

    let binance_url = format!("wss://stream.binance.com:9443/ws/{}@depth20@100ms",currency_pair);
    let (binance_ws_stream, _) = connect_async(binance_url).await.expect("Failed to connect");
    println!("Connected to binance");

    let (mut binance_write, mut binance_read) = binance_ws_stream.split();
    binance_subscribe(currency_pair.to_string(), &mut binance_write).await;

    let time = Duration::from_secs(5);
    //println!("Top 10 Bids: {:?}", top_bids);
    //println!("Top 10 Asks: {:?}", top_asks);
    //println!("Spread: {}", spread);

    loop {
        let bitstamp_data: Value = get_data_from_bitstamp(&mut bitstamp_read).await;
        let binance_data: Value = get_data_from_binance( &mut binance_read).await;
        let (spread, top_bids, top_asks) = merge_and_get_top(binance_data,bitstamp_data);
        let final_data = data_to_json(spread, top_bids, top_asks);
        println!("Final data: {:?}", final_data);
        tokio::time::sleep(time).await;
    }
}

fn data_to_json(spread: f64, top_bids: Vec<Order>, top_asks: Vec<Order>) -> Value {
    let bids_json: Vec<Value> = top_bids
        .iter()
        .map(|order| {
            json!({
                "price": order.price,
                "quantity": order.quantity,
                "exchange": order.exchange
            })
        })
        .collect();
    let asks_json: Vec<Value> = top_asks
        .iter()
        .map(|order| {
            json!({
                "price": order.price,
                "quantity": order.quantity,
                "exchange": order.exchange
            })
        })
        .collect();

    let output = json!({
        "spread": spread,
        "asks": asks_json,
        "bids": bids_json
    });
    output
}
fn merge_and_get_top(data1: Value, data2: Value) -> (f64, Vec<Order>, Vec<Order>) {
    let exchange1 = "binance";
    let exchange2 = "bitstamp";

    let mut bids: Vec<Order> = data1["bids"]
        .as_array()
        .expect("Failed to get bids from data1")
        .iter()
        .map(|entry| {
            let price = entry[0].as_f64().expect("Price should be a number");
            let quantity = entry[1].as_f64().expect("Quantity should be a number");
            Order {
                price,
                quantity,
                exchange: exchange1.to_string(),
            }
        })
        .chain(data2["data"]["bids"]
            .as_array()
            .expect("Failed to get bids from data2")
            .iter()
            .map(|entry| {
            let price = entry[0].as_f64().expect("Price should be a number");
            let quantity = entry[1].as_f64().expect("Quantity should be a number");
            Order {
                price,
                quantity,
                exchange: exchange2.to_string(),
            }
        }))
        .collect();

    let mut asks: Vec<Order> = data1["asks"]
        .as_array()
        .expect("Failed to get asks from first JSON")
        .iter()
        .map(|entry| {
            let price = entry[0].as_f64().expect("Price should be a number");
            let quantity = entry[1].as_f64().expect("Quantity should be a number");
            Order {
                price,
                quantity,
                exchange: exchange1.to_string(),
            }
        })
        .chain(data2["data"]["asks"]
            .as_array()
            .expect("Failed to get asks from second JSON")
            .iter()
            .map(|entry| {
            let price = entry[0].as_f64().expect("Price should be a number");
            let quantity = entry[1].as_f64().expect("Quantity should be a number");
            Order {
                price,
                quantity,
                exchange: exchange2.to_string(),
            }
        })
        )
        .collect();

    bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(Ordering::Equal));
    asks.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(Ordering::Equal));

    let top_bids = &bids[..10.min(bids.len())];
    let top_asks = &asks[..10.min(asks.len())];

    let highest_bid = top_bids.first().unwrap().price;
    let lowest_ask = top_asks.first().unwrap().price;

    let spread = highest_bid - lowest_ask;

    (spread, top_bids.to_vec(), top_asks.to_vec())
}

fn convert_string_numbers(json_value: &mut Value) {
    match json_value {
        Value::Array(arr) => {
            for item in arr {
                convert_string_numbers(item);
            }
        }
        Value::Object(map) => {
            for (_, value) in map {
                convert_string_numbers(value);
            }
        }
        Value::String(s) => {
            if let Ok(num) = s.parse::<f64>() {
                *json_value = Value::Number(serde_json::Number::from_f64(num).unwrap());
            }
        }
        _ => {}
    }
}

async fn bitstamp_subscribe(currency_pair: String, bitstamp_write: &mut SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>){
    let bitstamp_subscribe_json = json!({
        "event": "bts:subscribe",
        "data": {
            "channel": format!("order_book_{}",currency_pair)
        }
    });
    let bitstamp_subscribe_str = Message::Text(bitstamp_subscribe_json.to_string());
    bitstamp_write.send(bitstamp_subscribe_str).await.expect("Failed to subscribe");
}

async fn binance_subscribe(currency_pair: String, binance_write: &mut SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>) {
    let binance_subscribe_json = json!({
        "method": "SUBSCRIBE",
        "params": [
            format!("{}@aggTrade",currency_pair),
            format!("{}@depth",currency_pair)
        ],
        "id": 1
    });
    let binance_subscribe = Message::Text(binance_subscribe_json.to_string());

    binance_write.send(binance_subscribe).await.expect("Failed to subscribe");
}
async fn get_data_from_bitstamp(bitstamp_read: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>) -> Value {
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
                                    let mut parsed_data: Value = serde_json::from_str(&dat).expect("Failed to parse JSON");
                                    convert_string_numbers(&mut parsed_data);
                                    parsed_data
                                } else {
                                    Value::Null
                                }
                            } else {
                                Value::Null
                            }
                        } else {
                            Value::Null
                        }
                    } else {
                        Value::Null
                    }
                }else {
                    Value::Null
                }
            }
            Err(_e) => {
                Value::Null
            }
        }
    } else {
        Value::Null
    }
}

async fn get_data_from_binance(binance_read: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>) -> Value {
    if let Some(message) = binance_read.next().await {
        let message = message.expect("Failed to receive message");
        if let Message::Text(dat) = message {
            let mut parsed_data: Value = serde_json::from_str(&dat).expect("Failed to parse JSON");
            convert_string_numbers(&mut parsed_data);
            parsed_data
        } else {
            Value::Null
        }
    } else {
        Value::Null
    }
}