use chat_server::server::ChatServer;
use tokio_tungstenite::tungstenite::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let server = ChatServer::new();
    server.run("127.0.0.1:8080").await
}