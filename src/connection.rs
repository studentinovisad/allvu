use std::str::{from_utf8, Utf8Error};

use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use anyhow::{anyhow, Ok};

#[repr(u8)]
pub enum PacketType {
    InitialGreet = 1,
    NewSession = 2,
    ExistingSession = 3,
    ReadyForTransmission = 10,
    VideoStream = 20,
    CloseConnection = 100,
}

pub struct ConnectionPacket {
    pub packet_type: u8,
    pub packet_data: Vec<u8>
}

impl ConnectionPacket {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut packet_bytes: Vec<u8> = Vec::new();
        packet_bytes.push(self.packet_type);
        let packet_length: u32 = self.packet_data.len() as u32;
        let packet_length_bytes = u32::to_ne_bytes(packet_length);
        packet_bytes.append(&mut packet_length_bytes.to_vec());
        packet_bytes.append(&mut self.packet_data.clone());

        packet_bytes
    }

    pub fn to_string(&self) -> Result<&str, Utf8Error> {
        from_utf8(self.packet_data.as_slice())
    }
}

impl Clone for ConnectionPacket {
    fn clone(&self) -> Self {
        Self { 
            packet_type: self.packet_type.clone(), 
            packet_data: self.packet_data.clone() 
        }
    }
}

pub struct Connection {
    pub tcp_stream: TcpStream,
    pub penalty: u32
}

impl Connection {
    pub fn new(tcp_stream: TcpStream) -> Self {
        let connection = Self {
            tcp_stream,
            penalty: 0
        };
        connection
    }

    pub async fn read(&mut self) -> anyhow::Result<ConnectionPacket> {
        let mut packet_type_bytes = [0u8];
        let bytes_received = self.tcp_stream.read(&mut packet_type_bytes).await?;
        if bytes_received == 0 {
            return Err(anyhow!("Connection closed"));
        }

        let mut packet_size_bytes = [0u8; 4];
        self.tcp_stream.read(&mut packet_size_bytes).await?;
        let packet_size = u32::from_ne_bytes(packet_size_bytes);
        let mut packet_data: Vec<u8> = Vec::new();
        packet_data.resize(packet_size as usize, 0);
        self.tcp_stream.read(packet_data.as_mut_slice()).await?;

        Ok(ConnectionPacket {
            packet_type: packet_type_bytes[0],
            packet_data
        })
    }

    pub async fn write(&mut self, packet: ConnectionPacket) -> anyhow::Result<()> {
        let bytes = packet.to_bytes();
        self.tcp_stream.write_all(&bytes).await?;

        Ok(())
    }
}