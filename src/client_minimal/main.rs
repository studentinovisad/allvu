use std::{path::PathBuf, str::from_utf8, time::Duration};
use anyhow::anyhow;
use ffmpeg::FFmpeg;
use serde::Deserialize;
use serde_json::Value;
use tokio::{fs::read_to_string, process::Command, time::sleep};

#[path ="../ffmpeg.rs"]
mod ffmpeg;

async fn get_renderer() -> anyhow::Result<String> {
    let renderer_path = PathBuf::from("/dev/dri/");
    for dir_entry_result in renderer_path.read_dir()? {
        let Ok(dir_entry) = dir_entry_result else {
            continue;
        };
        let dev_name = String::from(dir_entry.file_name().to_str().unwrap_or(""));
        if dev_name.contains("render") {
            return Ok(String::from(dir_entry.path().to_str().unwrap()));
        }
    }

    Err(anyhow!("Renderer not found"))
}

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
    let config_path = PathBuf::from("allvu_client_minimal.toml");
    if !config_path.exists() {
        return Err(anyhow!("Config file not found"));
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

        let Ok(renderer_path) = get_renderer().await else {
            eprintln!("Couldn't get renderer, retrying...");
            sleep(Duration::from_secs(3)).await;
            continue;
        };
        println!("Renderer {renderer_path}");

        let mut ffmpeg_stream = FFmpeg::new();
        ffmpeg_stream.args = vec![
            "-vaapi_device".into(), renderer_path,
            "-f".into(), "video4linux2".into(),
            "-i".into(), camera_name,
            "-f".into(), "pulse".into(),
            "-i".into(), input_name,
            "-vf".into(), "format=nv12,hwupload".into(),
            "-c:v".into(), "h264_vaapi".into(),
            "-c:a".into(), "aac".into(),
            "-f".into(), "flv".into(),
            config.rtmp_server.clone()
        ];

        if let Err(e) = ffmpeg_stream.start() {
            eprintln!("Couldn't start FFmpeg {e}, retrying...");
            sleep(Duration::from_secs(3)).await;
            continue;
        }
        ffmpeg_stream.wait_until_end().await?;
    }

    Ok(())
}