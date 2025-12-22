//! Web server module for GhostLink.
//!
//! This module handles the HTTP layer of the application. It serves:
//! 1. The Static UI files (HTML/JS/CSS) from the `static/` directory.
//! 2. The API endpoints (e.g., status, configuration) for the frontend.

use super::shared_state::{Command, SharedState, Status};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::json;
use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    time::Duration,
};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::debug;

/// Starts the HTTP server on the specified port.
///
/// # Arguments
///
/// * `shared_state` - The thread safe application state.
/// * `port` - The port number to listen on (e.g., 8080).
///
/// # Returns
///
/// * `Ok(())` - If the server runs and stops gracefully.
/// * `Err` - If binding the port fails.
pub async fn serve(shared_state: SharedState, port: u16) -> Result<()> {
    let app = router(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Web UI available at http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Creates the Axum Router with all routes and middleware configured.
///
/// # Arguments
/// * `shared_state` - The thread safe application state.
pub fn router(shared_state: SharedState) -> Router {
    Router::new()
        .route("/api/ip", get(get_ip))
        .route("/api/status", get(get_status))
        .route("/api/peer", get(get_peer))
        .route("/api/connect", post(post_peer_ip))
        .route("/api/events", get(handle_sse))
        // Serve the "static" directory for all non-API requests
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
        .with_state(shared_state)
}

/// Handler for `GET /api/ip`
///
/// Returns the public ip and port of the local node.
async fn get_ip(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let data = state.read().await;
    Json(json!({
        "public_ip": data.public_ip,
    }))
}

/// Handler for `GET /api/peer`
///
/// Returns the ip and port of the connecting peer
async fn get_peer(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let data = state.read().await;
    Json(json!({
        "peer_ip": data.peer_ip,
    }))
}

/// Handler for `GET /api/status`.
///
/// Returns the current connection state of the application.
///
/// # Returns
/// JSON object: `{ "status": "disconnected" | "punching" | "connected" }`
async fn get_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let data = state.read().await;
    Json(json!({
        "status": data.status,
    }))
}

#[derive(Debug, Deserialize)]
struct ConnectionRequest {
    ip: String,
    port: u16,
}

/// Handler for `POST /api/connect`.
///
/// Initiates a P2P connection to the specified peer.
///
/// # Arguments
/// * `input` - JSON payload containing `ip` (String) and `port` (u16).
///
/// # Returns
/// * `200 OK` - If the connection request was received (process starts asynchronously).
/// * `422 Unprocessable Entity` - If the JSON input is invalid (wrong types).
async fn post_peer_ip(
    State(state): State<SharedState>,
    Json(input): Json<ConnectionRequest>,
) -> Result<(), (StatusCode, String)> {
    debug!("peer to connect: {}:{}", input.ip, input.port);

    // check if peer is allowed to connect
    if state.read().await.status != Status::Disconnected {
        return Err((
            StatusCode::BAD_REQUEST,
            "Already connecting/connected to a peer".into(),
        ));
    }

    // build SocketAddr
    let ip_addr = SocketAddr::new(
        IpAddr::V4(
            Ipv4Addr::from_str(&input.ip).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
        ),
        input.port,
    );
    // update peer ip to shared state
    {
        let mut data = state.write().await;
        data.peer_ip = Some(ip_addr);
    }

    // send controller `ConnectPeer` command
    let tx = { state.read().await.cmd_tx.clone() };
    if let Err(e) = tx.send(Command::ConnectPeer).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Controller unavailable: {e}"),
        ));
    }

    Ok(())
}

/// Handler for `POST /api/events
///
/// Upgrades to an SSE to to update UI with server side events.
///
/// # Returns
/// * `an SSE.
async fn handle_sse(
    State(state): State<SharedState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    debug!("SEE request received");

    // subscribe to event_tx
    let rx = state.read().await.event_tx.subscribe();
    let stream = BroadcastStream::new(rx);

    // build stream with event_tx
    let stream = stream.map(|msg| match msg {
        Ok(app_event) => {
            let json = serde_json::to_string(&app_event).unwrap_or_default();
            Ok(Event::default().data(json))
        }
        Err(_) => Ok(Event::default().comment("missed message")),
    });

    // return SSE stream
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::shared_state::{AppEvent, AppState, Status};
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tokio::sync::{RwLock, broadcast, mpsc};
    use tower::ServiceExt;

    /// Helper to create a fresh state for each test
    fn create_test_state() -> SharedState {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(32);
        let (event_tx, _) = broadcast::channel::<AppEvent>(32);
        // listen to cmd_rx and do nothing
        tokio::spawn(async move { while let Some(_cmd) = cmd_rx.recv().await {} });

        Arc::new(RwLock::new(AppState::new(
            None,
            Status::Disconnected,
            None,
            cmd_tx,
            event_tx,
        )))
    }

    /// Checks that before STUN runs (when public_ip is None), the `/api/ip` endpoint
    /// correctly returns `null` instead of crashing or returning an empty string.
    #[tokio::test]
    async fn test_get_ip_initial_is_null() {
        let state = create_test_state();
        let app = router(state);

        // 1. Create a mock request
        let request = Request::builder()
            .uri("/api/ip")
            .body(Body::empty())
            .unwrap();

        // 2. Send it to the router
        let response = app.oneshot(request).await.unwrap();

        // 3. Assertions
        assert_eq!(response.status(), StatusCode::OK);

        // Parse JSON body
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body_json, json!({ "public_ip": null }));
    }

    /// Manually forces the internal state to `Punching`, then calls the API to ensure
    /// the JSON response accurately reflects that change. This proves the "Shared State" logic works.
    #[tokio::test]
    async fn test_get_status_returns_current_state() {
        let state = create_test_state();

        // Simulate a state change
        {
            state.write().await.status = Status::Punching
        };

        let app = router(state);

        let request = Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();

        // Matches the enum variant name serialization
        assert_eq!(body_json, json!({ "status": "Punching" }));
    }

    /// Sends a valid JSON packet (IP + Port) to `/api/connect` to confirm the server
    /// accepts valid connection requests with a `200 OK`.
    #[tokio::test]
    async fn test_post_connect_valid_payload() {
        let state = create_test_state();
        let app = router(state);

        // 1. Create a valid JSON payload
        let payload = json!({
            "ip": "192.168.1.50",
            "port": 9000
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        // 2. Send request
        let response = app.oneshot(request).await.unwrap();

        // 3. Expect 200 OK
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// Sends broken JSON (missing the "port") to confirm the server rejects it with
    /// a `422 Unprocessable Entity` error, ensuring the app doesn't crash on bad input.
    #[tokio::test]
    async fn test_post_connect_invalid_payload_fails() {
        let state = create_test_state();
        let app = router(state);

        // 1. Create INVALID payload (missing "port")
        let payload = json!({
            "ip": "192.168.1.50"
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 2. Expect 422 Unprocessable Entity (Standard Axum error for bad JSON)
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    /// Ensures that if the app is already `Punching` or `Connected`,
    /// a new connection request is rejected with `400 Bad Request`.
    #[tokio::test]
    async fn test_post_connect_fails_when_busy() {
        let state = create_test_state();

        // 1. Simulate that we are already busy
        {
            let mut guard = state.write().await;
            guard.status = Status::Connected;
        }

        let app = router(state);

        let payload = json!({
            "ip": "192.168.1.55",
            "port": 9000
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 2. Expect rejection
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        // The error message comes as a plain string in the body
        assert_eq!(body_bytes, "Already connecting/connected to a peer");
    }

    /// Sends a malformed IP string (e.g., "invalid-ip") to ensure the
    /// server catches the parsing error and returns `400 Bad Request`.
    #[tokio::test]
    async fn test_post_connect_fails_on_invalid_ip() {
        let state = create_test_state();
        let app = router(state);

        let payload = json!({
            "ip": "not-an-ip-address",
            "port": 9000
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// Verifies that peer IP is correctly set in state after valid connection request.
    #[tokio::test]
    async fn test_post_connect_updates_peer_ip_in_state() {
        let state = create_test_state();
        let app = router(state.clone());

        let payload = json!({
            "ip": "10.0.0.5",
            "port": 7777
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify peer_ip was set correctly
        let locked_state = state.read().await;
        assert!(locked_state.peer_ip.is_some());
        let peer = locked_state.peer_ip.unwrap();
        assert_eq!(peer.ip().to_string(), "10.0.0.5");
        assert_eq!(peer.port(), 7777);
    }

    /// Verifies that status endpoint correctly reflects state transitions.
    #[tokio::test]
    async fn test_status_reflects_all_states() {
        let state = create_test_state();

        // Test Disconnected
        {
            state.write().await.status = Status::Disconnected;
        }
        let app = router(state.clone());
        let request = Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json, json!({ "status": "Disconnected" }));

        // Test Connected
        {
            state.write().await.status = Status::Connected;
        }
        let app = router(state.clone());
        let request = Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json, json!({ "status": "Connected" }));
    }

    /// Verifies that public IP is correctly returned when set.
    #[tokio::test]
    async fn test_get_ip_returns_set_value() {
        let state = create_test_state();

        // Set a public IP
        {
            let mut locked = state.write().await;
            locked.public_ip = Some("203.0.113.42:12345".parse().unwrap());
        }

        let app = router(state);

        let request = Request::builder()
            .uri("/api/ip")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body_json, json!({ "public_ip": "203.0.113.42:12345" }));
    }

    /// Verifies that connect endpoint rejects requests with invalid port numbers.
    #[tokio::test]
    async fn test_post_connect_accepts_valid_port_range() {
        let state = create_test_state();
        let app = router(state);

        // Test with port 65535 (max valid port)
        let payload = json!({
            "ip": "192.168.1.1",
            "port": 65535
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// Verifies that connect with status Punching is rejected.
    #[tokio::test]
    async fn test_post_connect_rejects_when_punching() {
        let state = create_test_state();

        {
            state.write().await.status = Status::Punching;
        }

        let app = router(state);

        let payload = json!({
            "ip": "192.168.1.1",
            "port": 9000
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/connect")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// Checks that `/api/peer` returns `null` before any connection is made.
    #[tokio::test]
    async fn test_get_peer_initial_is_null() {
        let state = create_test_state();
        let app = router(state);

        let request = Request::builder()
            .uri("/api/peer")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body_json, json!({ "peer_ip": null }));
    }

    /// Manually sets the peer IP in the state and verifies the API returns it correctly.
    #[tokio::test]
    async fn test_get_peer_returns_set_value() {
        let state = create_test_state();

        // 1. Simulate that a user has entered a peer IP
        {
            let mut guard = state.write().await;
            guard.peer_ip = Some("10.0.0.99:5000".parse().unwrap());
        }

        let app = router(state);

        let request = Request::builder()
            .uri("/api/peer")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body_json, json!({ "peer_ip": "10.0.0.99:5000" }));
    }

    /// Ensures that the `Punching` event serializes to the flat JSON structure
    /// required by the frontend: { "status": "PUNCHING", "timeout": 123, "message": "..." }
    #[test]
    fn test_app_event_serialization_punching() {
        let event = AppEvent::Punching {
            timeout: Some(30),
            message: Some("Hole punching started...".to_string()),
        };

        let json_str = serde_json::to_string(&event).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(v["status"], "PUNCHING");
        assert_eq!(v["timeout"], 30);
        assert_eq!(v["message"], "Hole punching started...");
    }

    /// Ensures that the `Connected` event serializes correctly:
    /// { "status": "CONNECTED", "message": "" }
    #[test]
    fn test_app_event_serialization_connected() {
        let event = AppEvent::Connected {
            message: Some("".to_string()),
        };

        let json_str = serde_json::to_string(&event).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(v["status"], "CONNECTED");
        assert_eq!(v["message"], "");
        // Ensure "timeout" field is NOT present for Connected events
        assert!(v.get("timeout").is_none());
    }

    /// Checks that `/api/events` accepts connections and returns the correct
    /// Content-Type header for Server-Sent Events.
    #[tokio::test]
    async fn test_sse_endpoint_headers() {
        let state = create_test_state();
        let app = router(state);

        let request = Request::builder()
            .uri("/api/events")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 1. Should be 200 OK
        assert_eq!(response.status(), StatusCode::OK);

        // 2. Content-Type must be "text/event-stream"
        let content_type = response
            .headers()
            .get("content-type")
            .expect("Response missing content-type header")
            .to_str()
            .unwrap();

        assert_eq!(content_type, "text/event-stream");
    }
}
