use std::{fs::read_dir, net::{SocketAddr, ToSocketAddrs}, path::PathBuf};
use anyhow::anyhow;
use clisession::{introduce_connection, ClientSession};
use ffmpeg::{AudioEncoder, Output, VideoEncoder};
use serde::Deserialize;
use tokio::{fs::read_to_string, net::TcpSocket};
use crate::{connection::{Connection, ConnectionPacket}, ffmpeg::FFmpeg};

#[path ="../connection.rs"]
mod connection;
#[path ="../ffmpeg.rs"]
mod ffmpeg;
#[path ="../session.rs"]
mod session;
mod clisession;

const ALLVU_PORT: u16 = 1312;
const ALLVU_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Config {
    server: String,
    camera: String
}

async fn get_config() -> anyhow::Result<Config> {
    let config_path = PathBuf::from("allvu_client.toml");
    if !config_path.exists() {
        return Err(anyhow!("Config file not found"));
    }

    let contents = read_to_string(config_path).await?;
    let config_file: Config = toml::from_str(&contents)?;
    Ok(config_file)
}

async fn get_network_interfaces() -> anyhow::Result<Vec<String>> {
    let mut interfaces: Vec<String> = Vec::new();

    let interfaces_folder = PathBuf::from("/sys/class/net");
    for possible_entry in read_dir(interfaces_folder)? {
        let Ok(dir_entry) = possible_entry else {
            continue;
        };

        let path = dir_entry.path();
        let carrier_path = path.join(PathBuf::from("carrier"));
        if path.join(PathBuf::from("device")).exists() && carrier_path.exists() {
            let name = dir_entry.file_name().into_string().unwrap();
            let carrier_info = read_to_string(carrier_path).await?;
            if carrier_info.chars().nth(0) == Some('1') {
                interfaces.push(name);
            }
        }
    }

    Ok(interfaces)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Client mode");

    let config = get_config().await?;
    let camera_path = config.camera;

    let server_address_str = format!("{}:1312", config.server);
    let mut server_addresses = server_address_str.to_socket_addrs().expect("Couldnt resolve server address");

    for addr in server_addresses.clone() {
        println!("{addr}");
    }
    
    let Some(server_address) = server_addresses.next() else {
        panic!("Server has no addresses");
    };

    let mut session = ClientSession::new();
    
    println!("Checking interfaces");
    let interfaces = get_network_interfaces().await?;
    println!("{:?}", interfaces);
    for interface_name in interfaces {
        let Ok(tcp_socket) = TcpSocket::new_v4() else {
            eprintln!("Couldn't create TcpSocket for {interface_name}");
            continue;
        };
        let Ok(_) = tcp_socket.bind_device(Some(interface_name.as_bytes())) else {
            eprintln!("Couldn't bind TcpSocket to {interface_name}");
            continue;
        };
        let Ok(tcp_stream) = tcp_socket.connect(server_address).await else {
            eprintln!("Couldn't connect to server from {interface_name}");
            eprintln!("{server_address}");
            continue;
        };
        println!("Connection created - {interface_name}");
        let mut connection = Connection::new(tcp_stream);
        introduce_connection(&mut connection).await?;
        session.add_connection(connection);
    }

    let mut camera_ffmpeg = FFmpeg::new();
    camera_ffmpeg.video_encoder = VideoEncoder::VAAPIH264;
    camera_ffmpeg.audio_encoder = AudioEncoder::AAC;
    camera_ffmpeg.output = Some(Output {
        path: "-".into(),
        output_type: ffmpeg::OutputType::FLV
    });

    camera_ffmpeg.start(vec![
        "-i", &camera_path
    ])?;

    loop {
        println!("reading ffmpeg");
        let Ok(bytes) = camera_ffmpeg.read().await else {
            eprintln!("Error reading from FFmpeg");
            continue;
        };
        println!("read {}", bytes.len());
        println!("{:?}", bytes);
        let packet = ConnectionPacket {
            packet_type: 20,
            packet_data: bytes
        };
        println!("writing conn packet");
        if let Err(e) = session.send(packet).await {
            eprintln!("Error sending to server {e}");
        }
        println!("written");
    }
}