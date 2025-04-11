use std::{path::PathBuf, process::Stdio, time::Duration};
use anyhow::anyhow;
use tokio::{fs::read_to_string, io::AsyncWriteExt, process::Command, time::sleep};


///
/// This command fixes a frozen camlink by unplugging it
/// and plugging it back into the computer. 
/// Sudo must be passwordless for the user running this
/// program.
/// 
pub async fn fix_camlink() -> anyhow::Result<()> {
    // Retrieve USB device with said name
    let usb_driver_path = PathBuf::from("/sys/bus/usb/drivers/usb");
    let mut camlink_device: Option<String> = None;
    for dir_entry_result in usb_driver_path.read_dir()? {
        let Ok(dir_entry) = dir_entry_result else {
            continue;
        };
        let dev_name = String::from(dir_entry.file_name().to_str().unwrap_or(""));

        let product_path = dir_entry.path().join("product");
        let port_path = dir_entry.path().join("port");
        if !port_path.exists() || !product_path.exists() {
            continue;
        }

        let product_name = read_to_string(product_path).await?;
        if product_name.contains("Cam Link") {
            camlink_device = Some(dev_name);
        }
    }

    let Some(device_port) = camlink_device else {
        return Err(anyhow!("Camlink not found"));
    };

    let mut unbind_command = Command::new("sudo")
        .args(vec![
            "tee", "/sys/bus/usb/drivers/usb/unbind"
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;

    unbind_command.stdin.as_mut().unwrap().write_all(device_port.as_bytes()).await?;

    // Waiting in order to be 100% sure that camlink power cycles properly
    sleep(Duration::from_secs(1)).await;
    unbind_command.kill().await?;

    let mut bind_command = Command::new("sudo")
        .args(vec![
            "tee", "/sys/bus/usb/drivers/usb/bind"
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;

    bind_command.stdin.as_mut().unwrap().write_all(device_port.as_bytes()).await?;

    // Wait for device to be properly initialized
    sleep(Duration::from_secs(1)).await;
    bind_command.kill().await?;

    Ok(())
}