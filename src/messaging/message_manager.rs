use super::{
    super::web::shared_state::{SharedState, Status},
    crypto::CipherAlgo,
    handshake::{self, HandshakeMsg},
};
use crate::config::EncryptionMode;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
};
use tokio_kcp::{KcpConfig, KcpNoDelayConfig, KcpStream};
use tracing::{error, info, warn};

/// Manages the lifecycle of a P2P connection, handling the transition from raw UDP to reliable KCP.
///
/// This struct is the central controller for:
/// 1. **Handshaking**: coordinating UDP hole punching via the `handshake` module.
/// 2. **Upgrading**: Converting the raw UDP socket into a reliable `KcpStream` without losing the underlying connection.
/// 3. **Teardown**: Safely closing the KCP stream while preserving the shared socket if needed.
#[derive(Debug)]
pub struct MessageManager {
    /// The shared UDP socket used for both initial discovery and the eventual KCP stream.
    client_socket: Arc<UdpSocket>,
    /// Shared application state for updating UI/Status.
    state: SharedState,
    /// The address of the connected peer. Only set after a successful handshake.
    peer_addr: Option<SocketAddr>,
    /// The active reliable stream. None until `upgrade_to_kcp` is called.
    kcp_stream: Option<KcpStream>,

    // --- ENCRYPTION FIELDS ---
    /// The session's encryption engine.
    cipher: Option<CipherAlgo>,
    /// Transmit nonce counter (strictly increasing).
    tx_nonce: u64,
    /// Receive nonce counter (strictly increasing).
    rx_nonce: u64,
}

/// Represents a message being sent/received to/from a peer.
#[derive(Serialize, Deserialize, Debug)]
pub enum StreamMessage {
    /// Regular chat content
    Text(String),
    /// Signal to close connection
    Bye,
}

impl MessageManager {
    /// Creates a new `MessageManager` in a disconnected state.
    ///
    /// # Arguments
    ///
    /// * `client_socket` - The local UDP socket bound to a specific port.
    /// * `state` - Reference to the application's shared state store.
    pub fn new(client_socket: Arc<UdpSocket>, state: SharedState) -> Self {
        Self {
            client_socket,
            state,
            peer_addr: None,
            kcp_stream: None,
            cipher: None, // Init
            tx_nonce: 0,  // Init
            rx_nonce: 0,  // Init
        }
    }

    /// Initiates the connection handshake with a target peer.
    ///
    /// This method blocks (async) until the handshake succeeds or times out.
    /// It handles the `Punching` -> `Connected` state transitions in the shared state.
    ///
    /// # Arguments
    ///
    /// * `peer_addr` - The public IP/Port of the target peer.
    /// * `timeout_secs` - Max time to wait for the handshake protocol to complete.
    /// * `mode` - Preferred encryption mode.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Handshake succeeded; `self.peer_addr` is set.
    /// * `Err` - Handshake failed; State reset to `Disconnected`.
    pub async fn handshake(
        &mut self,
        peer_addr: SocketAddr,
        timeout_secs: u64,
        mode: EncryptionMode,
    ) -> Result<()> {
        info!("Initializing MessageManager for peer {}", peer_addr);

        // Call the standalone handshake function with 5 arguments
        match handshake::handshake(
            self.client_socket.clone(),
            peer_addr,
            self.state.clone(),
            timeout_secs,
            mode,
        )
        .await
        {
            Ok(session) => {
                info!(
                    "Handshake successful. Session Key ID: {}",
                    session.fingerprint
                );
                self.peer_addr = Some(peer_addr);

                // Store the Cipher and Reset Nonces
                self.cipher = Some(session.cipher);
                self.tx_nonce = 0;
                self.rx_nonce = 0;

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

    /// Upgrades the existing raw UDP connection to a reliable KCP stream.
    ///
    /// This utilizes "Turbo Mode" configuration for low latency:
    /// - NoDelay: enabled
    /// - Update Interval: 10ms
    /// - Resend: 2 (fast retransmission)
    /// - No Congestion Control (NC): enabled
    /// - Windows: 1024 packets (allows higher throughput)
    /// - MTU: 1400 (safe default for UDP)
    ///
    /// # Errors
    ///
    /// Returns an error if the handshake has not been performed yet (`peer_addr` is None)
    /// or if the socket cloning fails.
    pub async fn upgrade_to_kcp(&mut self) -> Result<()> {
        if let Some(peer_addr) = self.peer_addr {
            info!("Upgrading connection to KCP with {}", peer_addr);

            // Configure KCP for low-latency
            let config = KcpConfig {
                nodelay: KcpNoDelayConfig {
                    nodelay: true,
                    interval: 10,
                    resend: 2,
                    nc: true,
                },
                wnd_size: (1024, 1024),
                mtu: 1400,
                ..Default::default()
            };

            // Safely clone the socket for KCP to take ownership of.
            let socket = self.clone_socket()?;

            // Connect the KCP stream wrapper.
            self.kcp_stream =
                Some(KcpStream::connect_with_socket(&config, socket, peer_addr).await?);

            info!("KCP upgrade successful.");
            Ok(())
        } else {
            bail!("Handshake not established.")
        }
    }

    /// Sends a text message wrapped in the StreamMessage protocol
    ///
    /// # Arguments
    ///
    /// * `text` - Message to send.
    pub async fn send_text(&mut self, text: String) -> Result<()> {
        let payload = bincode::serialize(&StreamMessage::Text(text))?;
        self.send_secure(&payload).await
    }

    /// Encrypts and sends a binary message over the established KCP stream.
    ///
    /// # Arguments
    ///
    /// * `payload` - The bytes to send.
    async fn send_secure(&mut self, payload: &[u8]) -> Result<()> {
        if let Some(stream) = &mut self.kcp_stream {
            if let Some(cipher) = &self.cipher {
                // Encrypt payload
                let ciphertext = cipher.encrypt(self.tx_nonce, payload)?;
                self.tx_nonce += 1;

                // Send ciphertext
                stream.write_all(&ciphertext).await?;
                stream.flush().await?;
                Ok(())
            } else {
                bail!("Encryption not initialized");
            }
        } else {
            bail!("KCP stream not established");
        }
    }

    /// Reads a message from the KCP stream, decrypts it, and writes to buffer.
    ///
    /// # Arguments
    ///
    /// * `buf` - The buffer to write received data into.
    ///
    /// # Returns
    ///
    /// * `Ok(usize)` - The number of bytes read.
    pub async fn receive_message(&mut self, buf: &mut [u8]) -> Result<usize> {
        if let Some(stream) = &mut self.kcp_stream {
            let n = stream.read(buf).await?;

            if n == 0 {
                return Ok(0);
            }

            if let Some(cipher) = &self.cipher {
                // Decrypt
                let ciphertext = &buf[..n];
                let plaintext = cipher.decrypt(self.rx_nonce, ciphertext)?;
                self.rx_nonce += 1;

                // Copy plaintext back to buf
                if plaintext.len() > buf.len() {
                    bail!("Buffer too small for plaintext");
                }
                buf[..plaintext.len()].copy_from_slice(&plaintext);

                Ok(plaintext.len())
            } else {
                bail!("Encryption not initialized");
            }
        } else {
            bail!("KCP stream not established");
        }
    }

    /// Returns true if the KCP stream is currently active.
    pub fn is_connected(&self) -> bool {
        self.kcp_stream.is_some()
    }

    /// Helper to clone the underlying UDP socket safely.
    ///
    /// `tokio-kcp` requires ownership of a `UdpSocket`, but we only have an `Arc<UdpSocket>`.
    /// This method uses `unsafe` code to duplicate the file descriptor (FD) and wrap it
    /// in a new `UdpSocket` struct.
    ///
    /// # Safety
    ///
    /// This method calls `std::mem::forget` on the temporary `std::net::UdpSocket`
    /// created from the raw FD. This is critical: if the temporary socket were dropped normally,
    /// it would close the FD, killing the original `Arc<UdpSocket>` as well.
    fn clone_socket(&self) -> Result<UdpSocket> {
        #[cfg(unix)]
        {
            use std::os::unix::io::{AsRawFd, FromRawFd};
            let fd = self.client_socket.as_raw_fd();

            // Create a std::net::UdpSocket from the raw fd.
            // must not drop this variable normally, it will close the fd Arc<UdpSocket> relies on.
            let std_sock = unsafe { std::net::UdpSocket::from_raw_fd(fd) };

            // try_clone() creates a new file descriptor (dup) that refers to the same socket.
            let new_std_sock = std_sock.try_clone();

            // Forget the original wrapper so the destructor doesn't fire and close the fd.
            std::mem::forget(std_sock);

            let new_std_sock = new_std_sock?;

            // Ensure the new socket is in non-blocking mode for Tokio
            new_std_sock.set_nonblocking(true)?;

            Ok(UdpSocket::from_std(new_std_sock)?)
        }

        #[cfg(not(unix))]
        {
            bail!("Socket cloning is currently only implemented for Unix-like systems.");
        }
    }

    /// Gracefully disconnects from the peer by sending a Bye message and cleaning up resources.
    ///
    /// This method:
    /// 1. Sends a Bye message to the peer (over KCP if connected, UDP as fallback)
    /// 2. Closes the KCP stream if active
    /// 3. Resets the connection state
    /// 4. Updates shared state to Disconnected
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Disconnection successful
    /// * `Err` - If sending the Bye message fails (cleanup still proceeds)
    pub async fn disconnect(&mut self) -> Result<()> {
        self.disconnect_internal(true).await
    }

    /// Disconnects from peer without sending Bye (used when receiving Bye from peer).
    ///
    /// This method performs cleanup without notifying the peer, since they already
    /// initiated the disconnect.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Disconnection successful
    pub async fn disconnect_on_bye_received(&mut self) -> Result<()> {
        self.disconnect_internal(false).await
    }

    /// Internal disconnect implementation with option to send Bye message.
    ///
    /// # Arguments
    ///
    /// * `send_bye` - If true, sends Bye message to peer before cleanup
    // FIXED: Added allow attribute here to fix linter error
    #[allow(clippy::collapsible_if)]
    async fn disconnect_internal(&mut self, send_bye: bool) -> Result<()> {
        info!("Initiating graceful disconnect (send_bye: {})", send_bye);

        // Send Bye message to peer only if requested
        if send_bye {
            if let Some(peer_addr) = self.peer_addr {
                let mut sent_via_kcp = false;

                // 1. Try KCP (Encrypted)
                if self.kcp_stream.is_some() && self.cipher.is_some() {
                    if let Ok(bye_packet) = bincode::serialize(&StreamMessage::Bye) {
                        if self.send_secure(&bye_packet).await.is_ok() {
                            info!("Sent Encrypted Bye via KCP");
                            sent_via_kcp = true;
                        }
                    }
                }

                // 2. Fallback: UDP Raw (HandshakeMsg::Bye)
                if !sent_via_kcp {
                    let udp_bye = bincode::serialize(&HandshakeMsg::Bye)?;
                    match self.client_socket.send_to(&udp_bye, peer_addr).await {
                        Ok(_) => info!("Sent HandshakeMsg::Bye via UDP"),
                        Err(e) => warn!("Failed to send Bye via UDP: {}", e),
                    }
                }
            }
        }

        // Close KCP stream if active
        if let Err(e) = self.close_kcp().await {
            warn!("Error closing KCP stream during disconnect: {}", e);
        }

        // Reset connection state
        self.peer_addr = None;
        // Reset Cipher
        self.cipher = None;
        self.tx_nonce = 0;
        self.rx_nonce = 0;

        // Clear chat history
        self.state.read().await.clear_chat();

        // Update shared state
        self.state.write().await.set_status(
            Status::Disconnected,
            Some("Disconnected from peer".into()),
            None,
        );

        info!("Disconnect complete");
        Ok(())
    }

    /// Closes the active KCP stream gracefully.
    ///
    /// This method:
    /// 1. Takes the stream out of the struct (setting `self.kcp_stream` to `None`).
    /// 2. Sends a termination signal (shutdown) to the peer.
    /// 3. Drops the stream, closing the *cloned* file descriptor.
    ///
    /// The original `client_socket` remains active.
    #[allow(dead_code)]
    pub async fn close_kcp(&mut self) -> Result<()> {
        if let Some(mut stream) = self.kcp_stream.take() {
            info!("Initiating KCP stream shutdown...");

            // Attempt graceful shutdown. Log errors but not fail the function
            if let Err(e) = stream.shutdown().await {
                error!("Error during KCP shutdown: {}", e);
            } else {
                info!("KCP stream shutdown completed.");
            }
            // Stream is dropped here, closing the cloned FD.
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::super::web::shared_state::{AppEvent, AppState, Command},
        *,
    };
    use std::os::unix::io::AsRawFd;
    use std::sync::Arc;
    use tokio::sync::{RwLock, broadcast, mpsc};

    /// Helper to create a fresh state for each test.
    fn create_test_state() -> SharedState {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(32);
        let (event_tx, _) = broadcast::channel::<AppEvent>(32);

        // Drain the command channel to prevent it from filling up during tests
        tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });

        Arc::new(RwLock::new(AppState::new(cmd_tx, event_tx)))
    }

    // Helper to create a dummy message manager with a bound socket
    async fn create_test_manager() -> MessageManager {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let state = create_test_state();
        MessageManager::new(Arc::new(socket), state)
    }

    #[tokio::test]
    async fn test_initialization() {
        let manager = create_test_manager().await;
        assert!(manager.peer_addr.is_none());
        assert!(manager.kcp_stream.is_none());
        assert!(!manager.is_connected());
        //crypto feat
        assert!(manager.cipher.is_none());
        assert_eq!(manager.tx_nonce, 0);
    }

    #[tokio::test]
    async fn test_upgrade_fails_without_handshake() {
        let mut manager = create_test_manager().await;

        // Should fail because peer_addr is None (handshake not run)
        let result = manager.upgrade_to_kcp().await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Handshake not established."
        );
    }

    #[tokio::test]
    async fn test_send_fails_without_kcp() {
        let mut manager = create_test_manager().await;
        let result = manager.send_text("hello".into()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_receive_fails_without_kcp() {
        let mut manager = create_test_manager().await;
        let mut buf = [0u8; 10];
        let result = manager.receive_message(&mut buf).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_close_kcp_safe_on_none() {
        let mut manager = create_test_manager().await;

        // Should return Ok even if stream is None (idempotent)
        let result = manager.close_kcp().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_socket_cloning_safety() {
        // This test verifies that clone_socket creates a NEW file descriptor
        // and does not close the original one when dropped.

        // 1. Setup socket and manager
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let raw_fd_original = socket.as_raw_fd();
        let socket_arc = Arc::new(socket);
        let manager = MessageManager::new(socket_arc.clone(), create_test_state());

        // 2. Clone the socket
        let cloned_sock = manager.clone_socket().expect("Socket cloning failed");
        let raw_fd_cloned = cloned_sock.as_raw_fd();

        // 3. Verify FDs are different (OS dup() should assign a new number)
        assert_ne!(
            raw_fd_original, raw_fd_cloned,
            "Cloned socket should have a different FD"
        );

        // 4. Drop the cloned socket explicitly
        drop(cloned_sock);

        // 5. Verify the original socket is still alive and working
        // If mem::forget was missed in implementation, this would fail/panic because the FD would be closed
        let test_payload = b"ping";
        // We send to ourselves just to check if the socket write operation fails immediately
        let send_result = socket_arc.send_to(test_payload, "127.0.0.1:8080").await;

        assert!(
            send_result.is_ok(),
            "Original socket should remain valid after cloned socket is dropped"
        );
    }

    #[tokio::test]
    async fn test_disconnect_without_connection() {
        let mut manager = create_test_manager().await;

        // Disconnect without being connected should work (idempotent)
        let result = manager.disconnect().await;
        assert!(result.is_ok());

        // Verify state was updated to Disconnected
        let state_guard = manager.state.read().await;
        assert_eq!(state_guard.status, Status::Disconnected);
    }

    #[tokio::test]
    async fn test_disconnect_with_peer_addr() {
        let mut manager = create_test_manager().await;

        // Set a peer address (simulating a connection)
        manager.peer_addr = Some("127.0.0.1:9999".parse().unwrap());

        // Disconnect
        let result = manager.disconnect().await;
        assert!(result.is_ok());

        // Verify peer_addr is cleared
        assert!(manager.peer_addr.is_none());

        // Verify state was updated to Disconnected
        let state_guard = manager.state.read().await;
        assert_eq!(state_guard.status, Status::Disconnected);
    }
}
