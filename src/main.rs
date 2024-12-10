use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{protocol::Message, Result},
};
use futures_util::{SinkExt, StreamExt};

struct ChatServer {
    // Store client information with username
    clients: Arc<Mutex<HashMap<String, (SocketAddr, broadcast::Sender<String>)>>>,
}

impl ChatServer {
    fn new() -> Self {
        ChatServer {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn handle_connection(
        &self,
        raw_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
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

        // Incoming message handler
        let clients_clone = Arc::clone(&self.clients);
        let username_clone = username.clone();
        tokio::spawn(async move {
            while let Some(message) = ws_receiver.next().await {
                if let Ok(Message::Text(text)) = message {
                    // Parse direct message
                    if text.starts_with('@') {
                        let parts: Vec<&str> = text.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            let target_username = parts[0].trim_start_matches('@');
                            let message_content = parts[1];

                            let clients = clients_clone.lock().await;
                            if let Some((_, target_tx)) = clients.get(target_username) {
                                let dm = format!("[DM from {}]: {}", username_clone, message_content);
                                let _ = target_tx.send(dm);
                            }
                        }
                    }
                }
            }
        });

        // Outgoing message handler
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if ws_sender.send(Message::text(msg)).await.is_err() {
                    break;
                }
            }
        });
    }

    async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server listening on: {}", addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let server = self.clone();
            
            tokio::spawn(async move {
                match accept_async(stream).await {
                    Ok(ws_stream) => {
                        server.handle_connection(ws_stream, addr).await;
                    }
                    Err(e) => eprintln!("Connection failed: {}", e),
                }
            });
        }

        Ok(())
    }
}

// Implement Clone for ChatServer
impl Clone for ChatServer {
    fn clone(&self) -> Self {
        ChatServer {
            clients: Arc::clone(&self.clients),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let server = ChatServer::new();
    server.run("127.0.0.1:8080").await
}