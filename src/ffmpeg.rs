use std::{io::Cursor, path::PathBuf, process::{ExitStatus, Stdio}};
use anyhow::{anyhow, Result};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, process::{Child, Command}, select, spawn, sync::oneshot};

const CHUNK_SIZE: usize = 500;

pub enum OutputType {
    FLV,
    MP4
}

pub struct Output {
    pub path: String,
    pub output_type: OutputType
}

pub enum VideoEncoder {
    SoftwareH264,
    VAAPIH264,
    Copy
}

pub enum AudioEncoder {
    AAC,
    Copy
}

pub struct FFmpeg {
    pub output: Option<Output>,
    pub video_encoder: VideoEncoder,
    pub audio_encoder: AudioEncoder,
    process: Option<Child>
}

fn get_vaapi_renderer() -> anyhow::Result<String> {
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

impl FFmpeg {
    pub fn new() -> Self {
        Self {
            process: None,
            output: None,
            video_encoder: VideoEncoder::VAAPIH264,
            audio_encoder: AudioEncoder::AAC,
        }
    }

    pub fn start(&mut self, args: Vec<&str>) -> Result<()> {
        // Initial args
        let mut combined_args: Vec<&str> = vec![
            "-hide_banner",
            "-loglevel",
            "error"
        ];

        // Program defined args
        combined_args.append(&mut args.clone());

        // Encoders
        let renderer_device: String;
        match self.video_encoder {
            VideoEncoder::VAAPIH264 => {
                renderer_device = get_vaapi_renderer()?;
                combined_args.append(&mut vec![
                    "-vaapi_device", &renderer_device,
                    "-vf", "format=nv12,hwupload",
                    "-c:v", "h264_vaapi",
                ]);
            }
            VideoEncoder::SoftwareH264 => {
                combined_args.append(&mut vec![
                    "-c:v", "libx264",
                ]);
            }
            VideoEncoder::Copy => {
                combined_args.append(&mut vec![
                    "-c:v", "copy",
                ]);
            }
        }

        match self.audio_encoder {
            AudioEncoder::AAC => {
                combined_args.append(&mut vec![
                    "-c:a", "aac",
                ]);
            }
            AudioEncoder::Copy => {
                combined_args.append(&mut vec![
                    "-c:a", "copy",
                ]);
            }
        }

        // Check if output is defined
        let Some(output) = &self.output else {
            return Err(anyhow!("Output is not defined"));
        };

        // Output type
        combined_args.push("-f");
        match output.output_type {
            OutputType::FLV => {
                combined_args.push("flv");
            }
            OutputType::MP4 => {
                combined_args.push("mp4");
            }
        }

        // Output path
        combined_args.push(&output.path);

        println!("ARGS {:?}", combined_args);

        // Start FFmpeg process
        let child_handle = Command::new("ffmpeg")
        .args(combined_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

        self.process = Some(child_handle);

        Ok(())
    }

    // Read and write functions

    pub async fn read(&mut self) -> Result<Vec<u8>> {
        let Some(process) = &mut self.process else {
            return Err(anyhow!("FFmpeg not started"));
        };

        let Some(stdout) = &mut process.stdout else {
            return Err(anyhow!("No stdout"));
        };

        let mut buffer = [0u8; CHUNK_SIZE];

        stdout.read_buf(&mut buffer.as_mut_slice()).await?;

        return Ok(Vec::from(buffer));
    }

    pub async fn write(&mut self, buffer: Vec<u8>) -> Result<()> {
        let Some(process) = &mut self.process else {
            return Err(anyhow!("FFmpeg not started"));
        };

        let Some(stdin) = &mut process.stdin else {
            return Err(anyhow!("No stdout"));
        };

        let mut cursor = Cursor::new(buffer);
        stdin.write_all_buf(&mut cursor).await?;

        return Ok(());
    }

    pub async fn wait_until_end(&mut self) -> anyhow::Result<ExitStatus> {
        let Some(process) = &mut self.process else {
            return Err(anyhow!("No process"));
        };
        let mut stderr = process.stderr.take().unwrap();
        
        let (stderr_tx, stderr_rx) = oneshot::channel::<()>();

        spawn(async move {
            let mut a = [0u8; 1];
            let _ = stderr.read(&mut a).await;
            let _ = stderr_tx.send(());
        });

        select! {
            exit_status = process.wait() => {
                println!("Process exited");
                return Ok(exit_status?);
            }
            _ = stderr_rx => {
                println!("Stderr outputted, exiting...");
                return Ok(ExitStatus::default());
            }
        }
    }
}