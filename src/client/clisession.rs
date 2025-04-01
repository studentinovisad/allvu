use crate::{connection::{Connection, ConnectionPacket, PacketType}, session::Session, ALLVU_VERSION};
use anyhow::anyhow;
use tokio::spawn;

pub struct ClientSession {
    session: Session
}

impl ClientSession {
    pub fn new() -> Self {
        let return_val = Self {
            session: Session::new()
        };

        return_val
    }

    pub fn add_connection(&mut self, connection: Connection)
    {
        self.session.add_connection(connection)
    }

    fn start_packet_processor(&self) {
        let packet_channel_arc = self.session.packet_channel.clone();
        spawn(async move {
            let receiver = &mut packet_channel_arc.1.lock().await;
            while let Some(packet) = receiver.recv().await {
                
            }
        });
    }

    pub async fn send(&self, packet: ConnectionPacket) -> anyhow::Result<()> {
        self.session.send(packet).await
    }
}

pub async fn introduce_connection(connection: &mut Connection) -> anyhow::Result<()> {
    let client_greet = format!("ALLVU-CLIENT-{ALLVU_VERSION}");
    let client_greet_bytes = client_greet.as_bytes().to_vec();
    let greet_packet = ConnectionPacket {
        packet_type: 1,
        packet_data: client_greet_bytes
    };

    println!("Sending to server...");

    connection.write(greet_packet).await?;
    let response_packet = connection.read().await?;
    let server_response = response_packet.to_string()?;

    let response_vec: Vec<&str> = server_response.split("-").collect();
    if response_vec.len() != 3 || response_vec[0] != "ALLVU" || response_vec[1] != "SERVER" {
        return Err(anyhow!("Not an AllVu server"));
    } else if response_vec[2] != ALLVU_VERSION {
        return Err(anyhow!("Server is not running the same version of AllVu"));
    }
    println!("Succesfully gotten response {server_response}");

    // TODO - ENCRYPTION

    // Retrieve token
    connection.write(
        ConnectionPacket { 
            packet_type: PacketType::NewSession as u8, 
            packet_data: "no password yet :)".as_bytes().to_vec()
        }
    ).await?;

    let token_packet = connection.read().await?;
    let token = token_packet.to_string()?;

    println!("Received token {token}");

    let mut is_server_ready = false;
    while !is_server_ready {
        println!("Is server ready?");
        let packet = connection.read().await?;
        if packet.packet_type == PacketType::ReadyForTransmission as u8 {
            is_server_ready = true;
        }
    }
    Ok(())
}