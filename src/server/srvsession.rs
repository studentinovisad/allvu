use tokio::spawn;

use crate::{connection::{Connection, ConnectionPacket, PacketType}, ffmpeg::FFmpeg, session::Session, ALLVU_VERSION};
use anyhow::anyhow;

pub struct ServerSession {
    session: Session,
    ffmpeg: FFmpeg
}

impl ServerSession {
    pub fn new() -> Self {
        let return_val = Self {
            session: Session::new(),
            ffmpeg: FFmpeg::new()
        };
        return_val.start_packet_processor();

        return_val
    }

    fn start_packet_processor(&self) {
        let packet_channel_arc = self.session.packet_channel.clone();
        spawn(async move {
            let receiver = &mut packet_channel_arc.1.lock().await;
            while let Some(packet) = receiver.recv().await {
                println!("Received packet of size {}", packet.packet_data.len());
                println!("Packet type {}", packet.packet_type);
            }
        });
    }

    pub fn add_connection(&mut self, connection: Connection)
    {
        self.session.add_connection(connection)
    }

    pub fn retreive_token(&mut self) -> String
    {
        self.session.retreive_token()
    }
}

pub enum IntroductionResult {
    NewSession(String),
    ExistingSession(String)
}

pub async fn introduce_connection(connection: &mut Connection) -> anyhow::Result<IntroductionResult> {
    let greet_packet = connection.read().await?;
    let client_greet = greet_packet.to_string()?;
    let greet_vec: Vec<&str> = client_greet.split("-").collect();
    if greet_vec.len() != 3 || greet_vec[0] != "ALLVU" || greet_vec[1] != "CLIENT" {
        return Err(anyhow!("Not an AllVu client"));
    } else if greet_vec[2] != ALLVU_VERSION {
        return Err(anyhow!("Client is not running the same version of AllVu"));
    }

    let server_response = format!("ALLVU-SERVER-{ALLVU_VERSION}");
    let server_response_bytes = server_response.as_bytes().to_vec();

    let response_packet = ConnectionPacket {
        packet_type: PacketType::InitialGreet as u8,
        packet_data: server_response_bytes
    };
    connection.write(response_packet).await?;

    let session_request_packet = connection.read().await?;
    println!("Gotten session request packet");
    println!("Packet type {}", session_request_packet.packet_type);
    if session_request_packet.packet_type == PacketType::NewSession as u8 {
        println!("New session");
        let password = session_request_packet.to_string()?;
        return Ok(IntroductionResult::NewSession(String::from(password)));
    } else if session_request_packet.packet_type == PacketType::ExistingSession as u8 {
        println!("Existing session");
        let session_token = session_request_packet.to_string()?;
        return Ok(IntroductionResult::ExistingSession(String::from(session_token)));
    }
    
    Err(anyhow!("Invalid packet received"))
}