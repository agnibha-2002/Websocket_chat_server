use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{protocol::Message, Result},
};

use super::handler::ConnectionHandler;

pub struct ChatServer {
    clients: Arc<Mutex<HashMap<String, (SocketAddr, broadcast::Sender<String>)>>>,
}

impl ChatServer {
    pub fn new() -> Self {
        ChatServer {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server listening on: {}", addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let server = self.clone();
            
            tokio::spawn(async move {
                match accept_async(stream).await {
                    Ok(ws_stream) => {
                        let handler = ConnectionHandler::new(server.clients.clone());
                        handler.handle_connection(ws_stream, addr).await;
                    }
                    Err(e) => eprintln!("Connection failed: {}", e),
                }
            });
        }

        Ok(())
    }
}

impl Clone for ChatServer {
    fn clone(&self) -> Self {
        ChatServer {
            clients: Arc::clone(&self.clients),
        }
    }
}