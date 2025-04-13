use std::{path::PathBuf, sync::Arc};
use serde::Deserialize;
use anyhow::anyhow;
use srvsession::{introduce_connection, IntroductionResult, ServerSession};
use tokio::{fs::read_to_string, net::TcpListener, sync::Mutex};
use crate::connection::{Connection, ConnectionPacket, PacketType};
use crate::session::Session;

#[path ="../connection.rs"]
mod connection;
#[path ="../ffmpeg.rs"]
mod ffmpeg;
#[path ="../session.rs"]
mod session;
mod srvsession;

const ALLVU_PORT: u16 = 1312;
const ALLVU_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Config {
    rtmp_output: String
}

async fn get_config() -> anyhow::Result<Config> {
    let config_path = PathBuf::from("allvu_server.toml");
    if !config_path.exists() {
        return Err(anyhow!("Config file not found"));
    }

    let contents = read_to_string(config_path).await?;
    let config_file: Config = toml::from_str(&contents)?;
    Ok(config_file)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Server mode");
    let listener = TcpListener::bind(format!("0.0.0.0:{ALLVU_PORT}")).await?;

    //let mut session: Session = Session::new();
    let mut sessions: Vec<Arc<Mutex<ServerSession>>> = Vec::new();
    
    loop {
        let (tcp_stream, _) = listener.accept().await?;
        
        let mut connection = Connection::new(tcp_stream);
        let introduction_result = introduce_connection(&mut connection).await?;
        match introduction_result {
            IntroductionResult::NewSession(password) => {
                println!("Creating new session...");
                let new_session = Arc::new(Mutex::new({
                    let mut session = ServerSession::new();
                    let token = session.retreive_token();
                    connection.write(ConnectionPacket { 
                        packet_type: 2, 
                        packet_data: token.as_bytes().to_vec()
                    }).await?;
                    session.add_connection(connection);
                    session
                }));
                sessions.push(new_session.clone());
            }
            IntroductionResult::ExistingSession(token) => {
                println!("Connecting client to existing session");
            }
        }
    }

    Ok(())
}