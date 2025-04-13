use std::{env, path::PathBuf, time::Duration};
use anyhow::anyhow;
use camlink_fixer::fix_camlink;
use ffmpeg::{AudioEncoder, FFmpeg, Output, VideoEncoder};
use input::{get_camera, get_input_source};
use serde::Deserialize;
use tokio::{fs::read_to_string, time::sleep};

#[path ="../ffmpeg.rs"]
mod ffmpeg;

#[path ="../camlink_fixer.rs"]
mod camlink_fixer;

#[path ="../input.rs"]
mod input;

#[derive(Deserialize)]
struct Config {
    rtmp_server: String,
    camera_pat: String,
    audio_pat: String,
    min_rate: Option<usize>,
    max_rate: Option<usize>
}

async fn get_config() -> anyhow::Result<Config> {
    let config_path = env::var("ALLVU_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("allvu_client_minimal.toml"));

    if !config_path.exists() {
        return Err(anyhow!("Config file not found at {:?}", config_path));
    }

    let contents = read_to_string(config_path).await?;
    let config_file: Config = toml::from_str(&contents)?;
    Ok(config_file)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("AllVu minimal client");
    let config = get_config().await?;

    loop {
        // CamLink fix - we're using them for camera input
        println!("Fixing camlink...");
        if let Err(e) = fix_camlink().await {
            eprintln!("Camlink fixing error {:?}", e);
        } else {
            println!("Camlink successfully fixed");
        } 

        let camera_name_result = get_camera(Some(&config.camera_pat)).await;
        let Ok(camera_name) = camera_name_result else {
            eprintln!("Couldn't get camera name {:?}, retrying...", camera_name_result.unwrap_err());
            sleep(Duration::from_secs(3)).await;
            continue;
        };
        println!("Camera path: {camera_name}");

        let Ok(input_name) = get_input_source(Some(&config.audio_pat)).await else {
            eprintln!("Couldn't get audio input name, retrying...");
            sleep(Duration::from_secs(3)).await;
            continue;
        };

        println!("PulseAudio input: {input_name}");

        let mut ffmpeg_stream = FFmpeg::new();
        ffmpeg_stream.video_encoder = VideoEncoder::VAAPIH264;
        ffmpeg_stream.audio_encoder = AudioEncoder::AAC;
        ffmpeg_stream.output = Some(Output {
            path: config.rtmp_server.clone(),
            output_type: ffmpeg::OutputType::FLV
        });

        let min_rate = format!("{}K", config.min_rate.unwrap_or(500));
        let max_rate = format!("{}K", config.max_rate.unwrap_or(4000));

        let ffmpeg_args = vec![
            "-f", "video4linux2",
            "-input_format", "yuyv422",
            "-framerate", "50",
            "-video_size", "1920x1080",
            "-i", &camera_name,
            "-f", "pulse",
            "-i", &input_name,
            "-b:v", max_rate.as_str(),
            "-minrate:v", min_rate.as_str(),
            "-maxrate:v", max_rate.as_str(),
            "-bufsize:v", "10M",
            "-preset", "fast",
        ];

        if let Err(e) = ffmpeg_stream.start(ffmpeg_args) {
            eprintln!("Couldn't start FFmpeg {e}, retrying...");
            sleep(Duration::from_secs(3)).await;
            continue;
        }
        
        ffmpeg_stream.wait_until_end().await?;
    }
}
