use super::{
    super::web::shared_state::{SharedState, Status},
    handshake,
};
use anyhow::{Result, bail};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::UdpSocket;
use tracing::{error, info};

/// Manages the lifecycle of a P2P connection.
///
/// This struct is responsible for:
/// 1. Initiating the connection handshake via the `handshake` module.
/// 2. Managing the active connection state (though currently it just sets up the connection).
/// 3. Handling failures and resetting the application state if the connection drops or fails.
#[derive(Debug)]
pub struct MessageManager {
    client_socket: Arc<UdpSocket>,
    state: SharedState,
}

impl MessageManager {
    /// Creates a new `MessageManager`.
    ///
    /// # Arguments
    ///
    /// * `client_socket` - The local UDP socket.
    /// * `state` - The shared application state.
    pub fn new(client_socket: Arc<UdpSocket>, state: SharedState) -> Self {
        Self {
            client_socket,
            state,
        }
    }

    /// The `handshake` function handles the `Punching` -> `Connected` state transitions
    /// and UI event broadcasting internally.
    ///
    /// # Arguments
    ///
    /// * `peer_addr` - The target peer's public address.
    /// * `timeout_secs` - Duration to wait for handshake before giving up.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the handshake is successful.
    /// * `Err` - If the handshake fails or times out. The state will be reset to `Disconnected`.
    pub async fn handshake(&mut self, peer_addr: SocketAddr, timeout_secs: u64) -> Result<()> {
        info!("Initializing MessageManager for peer {}", peer_addr);

        match handshake::handshake(
            self.client_socket.clone(),
            peer_addr,
            self.state.clone(),
            timeout_secs,
        )
        .await
        {
            Ok(_) => {
                info!("Handshake successful. MessageManager active.");
                Ok(())
            }
            Err(e) => {
                error!("Handshake failed: {}", e);

                self.state.write().await.set_status(
                    Status::Disconnected,
                    Some(format!("Connection failed: {}", e)),
                    None,
                );
                bail!(e);
            }
        }
    }
}
