//! Server — tokio-based TCP multi-client server with broadcast.

#[cfg(feature = "server")]
use {
    crate::PlatoEngine,
    std::sync::Arc,
    tokio::net::TcpListener,
    tokio::sync::{broadcast, Mutex},
    tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};

/// A handle to the running server's broadcast channel.
pub struct ServerHandle {
    tx: broadcast::Sender<String>,
}

impl ServerHandle {
    /// Broadcast a message to all subscribers.
    pub fn broadcast(&self, msg: String) {
        let _ = self.tx.send(msg);
    }
}

/// Start the Plato Engine Block TCP server.
///
/// Returns a tuple of (ServerHandle, join handle) so the caller can
/// broadcast tick results and shut down when done.
pub async fn run_server(
    engine: PlatoEngine,
    addr: &str,
) -> Result<(ServerHandle, tokio::task::JoinHandle<()>), Box<dyn std::error::Error + Send + Sync>>
{
    let engine = Arc::new(Mutex::new(engine));
    let (tx, _) = broadcast::channel(256);
    let listener = TcpListener::bind(addr).await?;

    let handle = ServerHandle { tx: tx.clone() };

    let join = tokio::spawn(async move {
        loop {
            let (socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => continue,
            };
            let engine = engine.clone();
            let tx = tx.clone();
            let mut rx = tx.subscribe();

            tokio::spawn(async move {
                let (reader, mut writer) = socket.split();
                let mut reader = BufReader::new(reader);
                let mut line = String::new();

                // Send welcome
                let _ = writer
                    .write_all(b"{\"type\":\"welcome\",\"room_id\":\"engine_room\",\"tick_hz\":0.2,\"sensors\":[]}\n")
                    .await;
                let _ = writer.flush().await;

                loop {
                    line.clear();
                    tokio::select! {
                        result = reader.read_line(&mut line) => {
                            match result {
                                Ok(0) => break, // EOF
                                Ok(_) => {}
                                Err(_) => break,
                            }
                            let cmd = line.trim();
                            if cmd.is_empty() {
                                continue;
                            }
                            let response = {
                                let mut eng = engine.lock().await;
                                eng.handle_command(cmd)
                            };
                            let _ = writer
                                .write_all(format!("{}\n", response).as_bytes())
                                .await;
                            let _ = writer.flush().await;
                        }
                        msg = rx.recv() => {
                            if let Ok(msg) = msg {
                                let _ = writer
                                    .write_all(format!("{}\n", msg).as_bytes())
                                    .await;
                                let _ = writer.flush().await;
                            }
                        }
                    }
                }
            });
        }
    });

    Ok((handle, join))
}
