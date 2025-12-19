//! Web server module for GhostLink.
//!
//! This module handles the HTTP layer of the application. It serves:
//! 1. The Static UI files (HTML/JS/CSS) from the `static/` directory.
//! 2. The API endpoints (e.g., status, configuration) for the frontend.

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, services::ServeDir};

/// Creates the Axum Router with all routes and middleware configured.
pub fn router() -> Router {
    Router::new()
        // Serve the "static" directory for all non-API requests
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive())
}

/// Starts the HTTP server on the specified port.
///
/// # Arguments
///
/// * `port` - The port number to listen on (e.g., 8080).
///
/// # Returns
///
/// * `Ok(())` - If the server runs and stops gracefully.
/// * `Err` - If binding the port fails.
pub async fn serve(port: u16) -> Result<()> {
    let app = router();

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Web UI available at http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
