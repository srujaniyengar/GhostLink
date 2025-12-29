mod config;
mod messaging;
mod net;
mod web;

use crate::{
    config::Config,
    messaging::message_manager::{MessageManager, StreamMessage},
    web::{
        shared_state::{AppEvent, AppState, Command, SharedState, Status},
        web_server,
    },
};
use anyhow::Result;
use std::sync::Arc;
use tokio::{
    net::UdpSocket,
    sync::{RwLock, broadcast, mpsc},
    time::Duration,
};
use tracing::{debug, error, info, warn};

/// Main entry point for the GhostLink application.
///
/// Initializes and starts:
/// 1. Logging system
/// 2. Configuration
/// 3. Communication channels
/// 4. Application state
/// 5. Web server
/// 6. Network controller
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    info!("Starting GhostLink v1.0");

    // Load configuration
    let config = Config::load();
    debug!("Configuration loaded successfully");

    // Create channels for communication between Web Server and Controller
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(32);
    let (event_tx, _event_rx) = broadcast::channel::<AppEvent>(32);

    // Initialize Shared State
    let shared_state = Arc::new(RwLock::new(AppState::new(cmd_tx.clone(), event_tx)));

    // Spawn Web Server
    let state_clone = Arc::clone(&shared_state);
    let web_server_handle = tokio::spawn(async move {
        let port = config.web_port;
        if let Err(e) = web_server::serve(state_clone, port).await {
            error!("Web server error: {:?}", e);
        }
    });

    // Spawn signal handler for graceful shutdown
    let cmd_tx_clone = cmd_tx.clone();
    let disconnect_timeout = config.disconnect_timeout_ms;
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl+C signal, initiating graceful shutdown");
                // Send disconnect command
                if let Err(e) = cmd_tx_clone.send(Command::Disconnect).await {
                    warn!("Failed to send disconnect command on shutdown: {}", e);
                }
                // Time for disconnect to complete
                tokio::time::sleep(Duration::from_millis(disconnect_timeout)).await;
                std::process::exit(0);
            }
            Err(e) => {
                error!("Failed to listen for Ctrl+C: {}", e);
            }
        }
    });

    // Start the main controller
    if let Err(e) = start_controller(&config, &shared_state, cmd_rx).await {
        error!("Controller error: {:?}", e);
    }

    // Wait for web server
    let _ = web_server_handle.await;

    Ok(())
}

/// Starts the main network controller.
///
/// Responsibilities:
/// 1. Binds UDP socket
/// 2. Resolves public IP via STUN
/// 3. Detects NAT type
/// 4. Handles commands and incoming messages
/// 5. Maintains NAT mappings via keep-alive
async fn start_controller(
    config: &Config,
    shared_state: &SharedState,
    mut cmd_rx: mpsc::Receiver<Command>,
) -> Result<()> {
    // 1. Bind the UDP Socket
    let socket = UdpSocket::bind(("0.0.0.0", config.client_port)).await?;
    let socket = Arc::new(socket);

    let local_port = socket.local_addr()?.port();
    info!("UDP socket bound to port {}", local_port);

    // 2. Resolve Local IP
    match net::get_local_ip(local_port).await {
        Ok(local_addr) => {
            info!("Local IP: {}", local_addr);
            shared_state.write().await.set_local_ip(
                local_addr,
                Some("Local IP resolved".into()),
                None,
            );
        }
        Err(e) => {
            warn!("Could not resolve local IP: {:?}", e);
        }
    }

    // 3. Resolve Public IP via STUN
    match net::resolve_public_ip(&socket, &config.stun_server).await {
        Ok(public_addr) => {
            info!("Public IP resolved via STUN: {}", public_addr);

            // Update state
            shared_state.write().await.set_public_ip(
                public_addr,
                Some("Public IP resolved".into()),
                None,
            );

            // 4. Detect NAT type
            let nat_type = net::get_nat_type(&socket, &config.stun_verifier, public_addr).await;
            shared_state.write().await.set_nat_type(
                nat_type,
                Some("NAT type detected".into()),
                None,
            );

            info!("NAT type: {:?}", nat_type);
        }
        Err(e) => {
            error!("STUN resolution failed: {:?}", e);
            warn!("Cannot accept incoming connections without public IP");
        }
    };

    let mut message_manager = MessageManager::new(Arc::clone(&socket), Arc::clone(shared_state));

    // 5. Command Loop with Keep-Alive
    info!("Ready to accept commands");

    let mut keep_alive_interval =
        tokio::time::interval(Duration::from_secs(config.punch_hole_secs));
    keep_alive_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut recv_buf = [0u8; 4096];

    loop {
        tokio::select! {
            // A. Handle Commands from Web
            cmd_opt = cmd_rx.recv() => {
                match cmd_opt {
                    Some(cmd) => {
                        match cmd {
                            Command::ConnectPeer => {
                                info!("Initiating peer connection");

                                // Read the target peer IP from state
                                let peer_ip_opt = shared_state.read().await.peer_ip;

                                if let Some(peer_addr) = peer_ip_opt {
                                    debug!("Connecting to peer: {}", peer_addr);

                                    if let Err(e) = message_manager.handshake(peer_addr, config.handshake_timeout_secs).await {
                                        error!("Handshake failed: {:?}", e);
                                    } else {
                                        info!("Connection established with peer");
                                        message_manager.upgrade_to_kcp().await?;
                                    }
                                } else {
                                    warn!("Cannot connect: no peer IP configured");
                                }
                            }

                            Command::SendMessage(msg) => {
                                if message_manager.is_connected() {
                                    match message_manager.send_text(msg.clone()).await {
                                        Ok(_) => {
                                            shared_state.read().await.add_message(msg, true);
                                        },
                                        Err(e) => error!("Message send failed: {}", e),
                                    }
                                } else {
                                    warn!("Cannot send message: not connected to peer");
                                }
                            }

                            Command::Disconnect => {
                                debug!("Disconnect command received");
                                if let Err(e) = message_manager.disconnect().await {
                                    error!("Disconnect failed: {:?}", e);
                                } else {
                                    info!("Successfully disconnected from peer");
                                }
                            }
                        }
                    }
                    None => {
                        debug!("Command channel closed, shutting down");
                        break;
                    }
                }
            }

            // B. Handle Incoming KCP Messages (Only if connected)
            res = message_manager.receive_message(&mut recv_buf), if message_manager.is_connected() => {
                match res {
                    Ok(n) => {
                        match bincode::deserialize::<StreamMessage>(&recv_buf[..n]) {
                            Ok(StreamMessage::Bye) => {
                                info!("Received BYE from peer. Disconnecting.");
                                let _ = message_manager.disconnect_on_bye_received().await;
                            }
                            Ok(StreamMessage::Text(content)) => {
                                shared_state.read().await.add_message(content, false);
                            }
                            Err(e) => {
                                warn!("Failed to deserialize KCP packet: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        error!("KCP stream error: {}", e);
                    }
                }
            }

            // C. Handle Keep-Alive (Heartbeat)
            _ = keep_alive_interval.tick() => {
                // Keep the NAT open if not connected.
                let status = shared_state.read().await.status;

                if status == Status::Disconnected {
                    debug!("Sending NAT keep-alive to STUN server");
                    // Re-resolving IP sends a STUN packet, which refreshes the NAT mapping.
                    match net::resolve_public_ip(&socket, &config.stun_server).await {
                        Ok(addr) => {
                            let mut state = shared_state.write().await;
                            if state.public_ip != Some(addr) {
                                info!("Public IP changed from {:?} to {}", state.public_ip, addr);
                                state.set_public_ip(addr, Some("Public IP updated".into()), None);
                            }
                        }
                        Err(e) => {
                            debug!("Keep-alive STUN check failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
