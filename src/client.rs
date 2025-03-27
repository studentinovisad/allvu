use std::path::PathBuf;
use aggligator_transport_tcp::simple::tcp_connect;
use anyhow::anyhow;
use serde::Deserialize;
use tokio::{fs::read_to_string, io::AsyncWriteExt};

use crate::{ffmpeg::FFmpeg, ALLVU_PORT};

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

pub async fn init() -> anyhow::Result<()> {
    println!("Client mode");

    let config = get_config().await?;
    let server_address = config.server;
    let camera_path = config.camera;

    let mut camera_ffmpeg = FFmpeg::new();
    camera_ffmpeg.args = vec![
        "-i".into(), camera_path,
        "-c".into(), "h264_qsv".into(),
        "-f".into(), "h264".into(),
        "-".into(),
    ];
    camera_ffmpeg.start().expect("Failed to start FFmpeg FLV");
    println!("Connecting...");

    let mut stream= tcp_connect(vec![server_address], ALLVU_PORT).await.expect("Failed to connect");

    println!("Transmitting...");
    
    loop {
        if let Ok(buffer) = camera_ffmpeg.read().await {
            if let Err(e) = stream.write_all_buf(&mut buffer.as_slice()).await {
                eprintln!("Error writing {:?}", e);
            }
        }
    }
}