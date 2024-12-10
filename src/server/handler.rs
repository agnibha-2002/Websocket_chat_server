use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::Message;

pub struct ConnectionHandler {
    clients: Arc<Mutex<HashMap<String, (SocketAddr, broadcast::Sender<String>)>>>,
}

impl ConnectionHandler {
    pub fn new(clients: Arc<Mutex<HashMap<String, (SocketAddr, broadcast::Sender<String>)>>>) -> Self {
        Self { clients }
    }

    pub async fn handle_connection(
        &self,
        raw_stream: WebSocketStream<TcpStream>,
        addr: SocketAddr,
    ) {
        let (mut ws_sender, mut ws_receiver) = raw_stream.split();

        // First message is username
        let username = match ws_receiver.next().await {
            Some(Ok(Message::Text(name))) => name.trim().to_string(),
            _ => return,
        };

        // Create a personal broadcast channel for this client
        let (tx, mut rx) = broadcast::channel(10);

        // Add client to global client list
        {
            let mut clients = self.clients.lock().await;
            clients.insert(username.clone(), (addr, tx.clone()));
        }

        // Handle incoming messages
        self.handle_incoming_messages(ws_receiver, username.clone()).await;

        // Handle outgoing messages
        self.handle_outgoing_messages(ws_sender, rx).await;
    }

    async fn handle_incoming_messages(
        &self,
        mut ws_receiver: futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
        username: String,
    ) {
        let clients = self.clients.clone();
        
        tokio::spawn(async move {
            while let Some(message) = ws_receiver.next().await {
                if let Ok(Message::Text(text)) = message {
                    if text.starts_with('@') {
                        let parts: Vec<&str> = text.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            let target_username = parts[0].trim_start_matches('@');
                            let message_content = parts[1];

                            let clients = clients.lock().await;
                            if let Some((_, target_tx)) = clients.get(target_username) {
                                let dm = format!("[DM from {}]: {}", username, message_content);
                                let _ = target_tx.send(dm);
                            }
                        }
                    }
                }
            }
        });
    }

    async fn handle_outgoing_messages(
        &self,
        mut ws_sender: futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
        mut rx: broadcast::Receiver<String>,
    ) {
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if ws_sender.send(Message::text(msg)).await.is_err() {
                    break;
                }
            }
        });
    }
}