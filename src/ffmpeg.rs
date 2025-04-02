use std::{io::Cursor, process::{ExitStatus, Stdio}};
use anyhow::{anyhow, Result};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, process::{Child, Command}};

const CHUNK_SIZE: usize = 500;

pub struct FFmpeg {
    pub args: Vec<String>,
    process: Option<Child>
}

impl FFmpeg {
    pub fn new() -> Self {
        Self {
            args: vec![],
            process: None,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let mut args: Vec<String> = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into()
        ];

        args.append(&mut self.args.clone());

        // Start FFmpeg process
        let child_handle = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        //.stderr(Stdio::null())
        .spawn()?;

        self.process = Some(child_handle);

        Ok(())
    }

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

        let exit_status = process.wait().await?;
        Ok(exit_status)
    }
}