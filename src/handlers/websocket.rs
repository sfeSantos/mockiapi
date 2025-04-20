use futures::{SinkExt, StreamExt};
use regex::Regex;
use warp::ws::{Message, WebSocket};
use crate::models::websocket::{WebSocketBehavior};

pub async fn handle_websocket(ws: WebSocket, behavior: WebSocketBehavior) {
    let (mut tx, mut rx) = ws.split();

    // Send on_connect messages
    if let Some(on_connect_msgs) = behavior.on_connect {
        for msg in on_connect_msgs {
            let _ = tx.send(Message::text(msg.text)).await;
        }
    }

    // Handle incoming messages
    while let Some(Ok(msg)) = rx.next().await {
        if let Ok(text) = msg.to_str() {
            if let Some(on_message_behaviors) = &behavior.on_message {
                for rule in on_message_behaviors {
                    let regex = Regex::new(&rule.match_regex).unwrap_or_else(|_| Regex::new(".*").unwrap());
                    if let Some(captures) = regex.captures(text) {
                        let mut response = rule.response.text.clone();
                        // Replace capture groups $0, $1, etc.
                        for (i, cap) in captures.iter().enumerate() {
                            if let Some(cap_match) = cap {
                                response = response.replace(&format!("${}", i), cap_match.as_str());
                            }
                        }
                        let _ = tx.send(Message::text(response)).await;
                        break; // Only respond to the first match
                    }
                }
            }
        }
    }
}