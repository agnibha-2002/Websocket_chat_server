// Step 1: Importing necessary dependencies
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

// Step 2: Create a shared state for managing connections
struct ChatServer {
    // Broadcast channel for sending messages to all clients
    tx: broadcast::Sender<String>,
    
    // Track connected client addresses
    clients: Arc<Mutex<HashMap<SocketAddr, broadcast::Sender<String>>>>,
}

impl ChatServer {
    // Constructor method
    fn new() -> Self {
        // Create a broadcast channel with a buffer of 10 messages
        let (tx, _) = broadcast::channel(10);
        
        ChatServer {
            tx,
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Method to handle individual client connections
    async fn handle_connection(
        &self,
        raw_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        addr: SocketAddr,
    ) {
        // Split the WebSocket stream into sender and receiver
        let (mut ws_sender, mut ws_receiver) = raw_stream.split();

        // Create a receiver for this specific client
        let rx = self.tx.subscribe();

        // Spawn a task to handle incoming messages
        tokio::spawn({
            let tx = self.tx.clone();
            let clients = Arc::clone(&self.clients);
            
            async move {
                // Add this client to the tracked connections
                {
                    let mut client_map = clients.lock().await;
                    client_map.insert(addr, tx.clone());
                }

                // Receive messages from the client
                while let Some(message) = ws_receiver.next().await {
                    match message {
                        Ok(msg) => match msg {
                            Message::Text(text) => {
                                // Broadcast the message to all clients
                                let _ = tx.send(format!("{}: {}", addr, text));
                            }
                            Message::Close(_) => break,
                            _ => {}
                        },
                        Err(_) => break,
                    }
                }

                // Remove client when disconnected
                let mut client_map = clients.lock().await;
                client_map.remove(&addr);
            }
        });

        // Spawn a task to handle outgoing messages
        tokio::spawn({
            let mut rx = rx;
            async move {
                while let Ok(msg) = rx.recv().await {
                    if ws_sender.send(Message::text(msg)).await.is_err() {
                        break;
                    }
                }
            }
        });
    }

    // Main server run method
    async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Chat server listening on: {}", addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let server = self.clone();
            
            // Spawn a new task for each WebSocket connection
            tokio::spawn(async move {
                // Add proper WebSocket upgrade headers
                match accept_async(stream).await {
                    Ok(ws_stream) => {
                        println!("New WebSocket connection: {}", addr);
                        server.handle_connection(ws_stream, addr).await;
                    }
                    Err(e) => eprintln!("Failed to accept WebSocket connection: {}", e),
                }
            });
        }

        Ok(())
    }
}

// Implement Clone for ChatServer to allow easy spawning
impl Clone for ChatServer {
    fn clone(&self) -> Self {
        ChatServer {
            tx: self.tx.clone(),
            clients: Arc::clone(&self.clients),
        }
    }
}

// Async main entry point
#[tokio::main]
async fn main() -> Result<()> {
    let server = ChatServer::new();
    match server.run("127.0.0.1:8080").await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Server error: {}", e);
            Err(e)
        }
    }
}