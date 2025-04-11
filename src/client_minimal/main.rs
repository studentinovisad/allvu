use std::{env, path::PathBuf, str::from_utf8, time::Duration};
use anyhow::anyhow;
use camlink_fixer::fix_camlink;
use ffmpeg::{AudioEncoder, FFmpeg, Output, VideoEncoder};
use serde::Deserialize;
use serde_json::Value;
use tokio::{fs::read_to_string, process::Command, time::sleep};

#[path ="../ffmpeg.rs"]
mod ffmpeg;

#[path ="../camlink_fixer.rs"]
mod camlink_fixer;

async fn get_camera(pat: &str) -> anyhow::Result<String> {
    let v4l2_path = PathBuf::from("/sys/class/video4linux/");
    let mut matching_devices: Vec<String> = Vec::new();
    for dir_entry_result in v4l2_path.read_dir()? {
        let Ok(dir_entry) = dir_entry_result else {
            continue;
        };
        let dev_name = String::from(dir_entry.file_name().to_str().unwrap_or(""));
        let name_path = dir_entry.path().join("name");
        if !name_path.exists() {
            continue;
        }

        let camera_name = read_to_string(name_path).await?;
        if camera_name.contains(pat) {
            let mut dev_path = String::from("/dev/");
            dev_path.push_str(&dev_name);
            matching_devices.push(dev_path);
        }
    }

    if matching_devices.len() == 0 {
        return Err(anyhow!("Not found"));
    }

    matching_devices.sort();
    Ok(matching_devices[0].clone())
}

#[derive(Deserialize)]
struct Config {
    rtmp_server: String,
    camera_pat: String,
    audio_pat: String
}

async fn get_input_source(pat: &str) -> anyhow::Result<String> {
    let cmd = Command::new("pactl")
        .args(["-f", "json", "list", "short", "sources"])
        .output().await?;

    let output_str = from_utf8(&cmd.stdout)?;
    let output_json: Value = serde_json::from_str(output_str)?;
    let Some(sources) = output_json.as_array() else {
        return Err(anyhow!("Couldn't get sources from json"));
    }; 

    for audio_source_value in sources {
        let Some(audio_source) = audio_source_value.as_object() else {
            continue;
        };
        
        let Some(name_value) = audio_source.get("name") else {
            continue;
        };

        let Some(source_name) = name_value.as_str() else {
            continue;
        };
        
        if source_name.contains(pat) {
            return Ok(String::from(source_name));
        }
    }

    Err(anyhow!("No input found"))
    //
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

        let camera_name_result = get_camera(&config.camera_pat).await;
        let Ok(camera_name) = camera_name_result else {
            eprintln!("Couldn't get camera name {:?}, retrying...", camera_name_result.unwrap_err());
            sleep(Duration::from_secs(3)).await;
            continue;
        };
        println!("Camera path: {camera_name}");

        let Ok(input_name) = get_input_source(&config.audio_pat).await else {
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

        let ffmpeg_args = vec![
            "-f", "video4linux2",
            "-input_format", "yuyv422",
            "-framerate", "50",
            "-video_size", "1920x1080",
            "-i", &camera_name,
            "-f", "pulse",
            "-i", &input_name,
            "-maxrate", "5M",
            "-bufsize", "1M",
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
