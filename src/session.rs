use std::sync::{atomic::AtomicU32, Arc};

use rand::distr::{Alphanumeric, SampleString};
use tokio::{spawn, sync::{mpsc::{self, Sender, Receiver}, Mutex}};

use crate::connection::{Connection, ConnectionPacket, PacketType};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

pub struct Session {
    id: u32,
    token: String,
    connections: Vec<Arc<Mutex<Connection>>>,
    pub packet_channel: Arc<(Sender<ConnectionPacket>, Mutex<Receiver<ConnectionPacket>>)>,
    // Load balancing fields
    lb_index: u8
}

impl Session {
    pub fn new() -> Self {
        let session_token = Alphanumeric.sample_string(&mut rand::rng(), 24);
        let packet_channel = {
            let mpsc_channel = mpsc::channel(64);
            Arc::new((mpsc_channel.0, Mutex::new(mpsc_channel.1)))
        };
        let to_return = Self {
            id: NEXT_ID.fetch_add(1u32, std::sync::atomic::Ordering::AcqRel),
            token: session_token,
            connections: vec![],
            packet_channel,
            lb_index: 0
        };

        to_return
    }

    pub fn add_connection(&mut self, connection: Connection) {
        let connection_arc = Arc::new(Mutex::new(connection));
        self.connections.push(connection_arc.clone());
        let packet_channel_arc = self.packet_channel.clone();
        spawn(async move {
            let mut sent_ready = false;
            loop {
                let mut lock = connection_arc.lock().await;
                if !sent_ready {

                    let Ok(_) = (*lock).write(
                        ConnectionPacket { 
                            packet_type: PacketType::ReadyForTransmission as u8, 
                            packet_data: "AllVu Ready".as_bytes().to_vec()
                        }
                    ).await else {
                        continue;
                    };
                    sent_ready = true;
                }
                let Ok(packet) = (*lock).read().await else {
                    continue;
                };
                drop(lock);
                let sender = &packet_channel_arc.0;
                sender.send(packet).await;
            }
        });
    }

    pub fn retreive_token(&self) -> String {
        self.token.clone()
    }

    pub async fn send(&self, packet: ConnectionPacket) -> anyhow::Result<()> {
        let mut lock = self.connections[0].lock().await;
        lock.write(packet).await?;
        Ok(())
    }
}