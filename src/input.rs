use std::{path::PathBuf, str::from_utf8};
use anyhow::anyhow;
use serde_json::Value;
use tokio::{fs::read_to_string, process::Command};

pub async fn get_camera(pat: Option<&str>) -> anyhow::Result<String> {
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
        if pat.is_none_or(|pat| camera_name.contains(pat)) {
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

pub async fn get_input_source(pat: Option<&str>) -> anyhow::Result<String> {
    let Some(name_pat) = pat else {
        return Ok("default".into());
    };
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
        
        if source_name.contains(name_pat) {
            return Ok(String::from(source_name));
        }
    }

    Err(anyhow!("No input found"))
    //
}