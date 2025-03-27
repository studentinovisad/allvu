use text_io::read;

const ALLVU_PORT: u16 = 1312;
const CHUNK_SIZE: usize = 1024;

mod ffmpeg;
mod server;
mod client;

#[tokio::main]
async fn main() {
    println!("AllVu proof of concept");
    print!("Enter AllVu type (S,C): ");
    let alvu_type: String = read!();
    if alvu_type.to_lowercase() == "s" {
        server::init().await.expect("Server failed");
    } else {
        client::init().await.expect("Client failed");
    }
}