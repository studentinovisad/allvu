use std::{net::{Ipv4Addr, SocketAddr}, path::PathBuf};
use aggligator_transport_tcp::simple::tcp_server;
use serde::Deserialize;
use anyhow::anyhow;
use tokio::{fs::read_to_string, io::AsyncReadExt};

use crate::{ffmpeg::FFmpeg, ALLVU_PORT, CHUNK_SIZE};

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

pub async fn init() -> anyhow::Result<()> {
    println!("Server mode");

    tcp_server(
        SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), ALLVU_PORT), 
        |mut stream| async move {
            println!("Stream retrieved");
            println!("{:?}", stream.id());
            println!("Listening...");

            let config = get_config().await.unwrap();

            let mut rtmp_ffmpeg = FFmpeg::new();
            rtmp_ffmpeg.args = vec![
                "-f".into(), "h264".into(),
                "-i".into(), "-".into(),
                "-c".into(), "libx264".into(),
                "-preset".into(), "ultrafast".into(),
                "-threads".into(), "0".into(),
                "-f".into(), "flv".into(),
                config.rtmp_output
            ];
            rtmp_ffmpeg.start().expect("Failed to start FFmpeg");

            loop {
                let mut buffer = [0u8; CHUNK_SIZE];
                let received = stream.read_buf(&mut buffer.as_mut_slice()).await.unwrap();
                println!("Received {received} bytes, writing to ffmpeg...");
                if let Err(e) = rtmp_ffmpeg.write(Vec::from(buffer)).await {
                    eprintln!("Error writing to ffmpeg {:?}", e);
                }
            }
            
        }
    ).await.expect("Server failed"); 
    println!("OK");

    Ok(())
}