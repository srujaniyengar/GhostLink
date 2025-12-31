<div align="center">
  <img src="./static/banner.png" alt="GhostLink Banner" width="100%">
  
  <a href="https://github.com/pushkar-gr/ghostlink/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/pushkar-gr/ghostlink/ci.yml?label=Build&style=for-the-badge&logo=github" alt="Build Status">
  </a>
  <a href="https://github.com/pushkar-gr/ghostlink/releases">
    <img src="https://img.shields.io/github/v/release/pushkar-gr/ghostlink?label=Version&style=for-the-badge&color=007ec6" alt="Latest Version">
  </a>
  <a href="https://github.com/pushkar-gr/ghostlink/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/License-GPLv3-blue.svg?style=for-the-badge" alt="License">
  </a>
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/Made%20With-Rust-orange?style=for-the-badge&logo=rust" alt="Language">
  </a>
</div>

# 👻 GhostLink

**High-Performance Serverless P2P Messaging**

GhostLink is a decentralized chat application engineered for direct peer-to-peer communication. By leveraging UDP hole punching and a custom reliability layer, it eliminates the need for central relay servers, ensuring messages travel the shortest path possible between peers.

This project prioritizes low-latency networking, asynchronous runtime efficiency, and clean architecture.

---

## 🗺️ Roadmap & Status

| Version | Status       | Description                                                                                                   |
|---------|--------------|---------------------------------------------------------------------------------------------------------------|
| v1.0    | **Stable**   | Current release. Feature complete for text exchange and reliable UDP transport.                              |
| v1.1    | **In Progress** | Security Hardening (QUIC). Replacing the KCP stack with QUIC (via `quinn`) to implement TLS 1.3 End-to-End Encryption. |

---

## ⚙️ Architectural Overview

GhostLink operates by decoupling the Web Interface from the P2P Networking Core, bridging them via shared thread-safe state.

[![](https://mermaid.ink/img/pako:eNqFVFtv2jAU_iuWHyYqESBcSskQUkt3QYOWNUVIW_ZgklNikdiZ7YzSqv99xwnXdWrzdI59vu87t_iZhjIC6lHHcQIRSvHAl14gCDExpOARAdKJmFoFogh4SOQ6jJkyZHyHUTpfLBXLYmt8sUbjZ0CnCjQIwwyXgozZBhSpzEZnAf1lectvDguM7C8GaJCJjPIE-vXFoL9Qg8vHPK3PmcqID-oPqC0ORPQ_QRdphlIYJRMyTZgAUrmPgVwpxsWp5DYqAVUqTzDi6HAn72PiQCYsjDmSfWBp9pEMYyYEJPqEbwJasyUXyy3dziVjueThjg6hhnzl2ki12bJ9liplxmDo26U1kfheMaEzaftdNvIOEs4WPOFmc1qefz-7KTOxVtE7Hu6bes11KLGZmxPMt-G0hKBB5qiZHfpwJLTN-1ZFoN7NuoWMN2DWUq32ObM1GdVv_8lXhiswpfzserr1yR1omatD4n6GGRCfi1XdNwpY-rZ6Gwm_xFKbMSLIpcIpGghNruBEfL-ur4_c10fN10etQw7lNpO-4-z3BPddY-FDmaZMRPqMOM7gaNVK0MHHa6z7dw7akNGUVK6kUnK97UgJtjMtYcV0nRr5nuM0Sc3eFXHHt5YOaxaW7X3pkeCGs4Q_AW6N1vavtaD9QpeYw37bSq82BnQRhqtTBtgdsld22lNmUyoDjtM7FZ5lEbM0Fd__VJ_7ZaXYS1qlS8Uj6hmVQ5WmgP-LdemzZQlo8TQF1EOzeJpoIF4QkzHxQ8p0B1MyX8bUe2CJRi8vtK45w305hOAEQQ1lLgz1Wm5BQb1n-kg9t9eqNc4vXLfTbLVb3U6VbqjnNBu13oXbcbvdntttXzTa3ZcqfSpE3VoDMW6v0eueu63OebtKIeL420_K97V8WunLX189u5E?type=png)](https://mermaid.live/edit#pako:eNqFVFtv2jAU_iuWHyYqESBcSskQUkt3QYOWNUVIW_ZgklNikdiZ7YzSqv99xwnXdWrzdI59vu87t_iZhjIC6lHHcQIRSvHAl14gCDExpOARAdKJmFoFogh4SOQ6jJkyZHyHUTpfLBXLYmt8sUbjZ0CnCjQIwwyXgozZBhSpzEZnAf1lectvDguM7C8GaJCJjPIE-vXFoL9Qg8vHPK3PmcqID-oPqC0ORPQ_QRdphlIYJRMyTZgAUrmPgVwpxsWp5DYqAVUqTzDi6HAn72PiQCYsjDmSfWBp9pEMYyYEJPqEbwJasyUXyy3dziVjueThjg6hhnzl2ki12bJ9liplxmDo26U1kfheMaEzaftdNvIOEs4WPOFmc1qefz-7KTOxVtE7Hu6bes11KLGZmxPMt-G0hKBB5qiZHfpwJLTN-1ZFoN7NuoWMN2DWUq32ObM1GdVv_8lXhiswpfzserr1yR1omatD4n6GGRCfi1XdNwpY-rZ6Gwm_xFKbMSLIpcIpGghNruBEfL-ur4_c10fN10etQw7lNpO-4-z3BPddY-FDmaZMRPqMOM7gaNVK0MHHa6z7dw7akNGUVK6kUnK97UgJtjMtYcV0nRr5nuM0Sc3eFXHHt5YOaxaW7X3pkeCGs4Q_AW6N1vavtaD9QpeYw37bSq82BnQRhqtTBtgdsld22lNmUyoDjtM7FZ5lEbM0Fd__VJ_7ZaXYS1qlS8Uj6hmVQ5WmgP-LdemzZQlo8TQF1EOzeJpoIF4QkzHxQ8p0B1MyX8bUe2CJRi8vtK45w305hOAEQQ1lLgz1Wm5BQb1n-kg9t9eqNc4vXLfTbLVb3U6VbqjnNBu13oXbcbvdntttXzTa3ZcqfSpE3VoDMW6v0eueu63OebtKIeL420_K97V8WunLX189u5E)

### Communication Flow

[![](https://mermaid.ink/img/pako:eNqVVsFu20YQ_ZXpnmxUZEnKsiweXJB2ERupBcK0EaRVUWzIkUSY5LK7yzSq4d5yyye0P5cv6SxJhZIluYkOAld8M_Pe25ldPbJEpMh8pvCPGssELzO-kLyYlUAfXmtR1sU7lO26_a641FmSVbzUEABXcK9Q0tPRq6VQ-uesfDjeBU6Du98bMD0Y7K2oNco9wNiAXgmxyBHiu_vp_lThOlX4QqrwC7lwm1wLnQqNIN4b7oPQh2jJFYLrw3WKpc70Ci4zlZj3KzgyTI43TQis8_PvYx_eLAXwAq5_hKMwK9OsXMCtsVLpDh5bhLQCH96KGrhE8Jyh7diuO7Rdf-Q4DhxF9bs8S-A62qoQfluFsK_gTs7skWu7jmN7_ulOib3qjU0KElGtrIorenUdKSh4WfM8X8H7jLduyPSH-CZ-0UHPhzgr6lzzEkWt4ErQTkZ1mSyJ-nMLg7YyJMTuQcGMXYiyxETPWG9C-AKmReVCVPBTs1PkaKGM3rYexJpr7Izq-gMibPo10BqLSqv-5ZpV06wkA8uUysVvpzMGWkC4jWxQHdpwvIwg4skDamoXmfiGijOAS6V9MJtw_CzaeCezxVKDmENXsR2ORCKRViDrHH0I8lz82WQA67xJ-n-JiMznfz9145FKUSmS3RKbiiYrzIVxYIV6gxUntY014QFrwl7sc2uCXWvC3sgda057a0YHrMlxvimoFbPPmVHnzOl-ZzbzBL0xwTNjbnmC1P40XzoT5QYhEtoumoe9fW_bNgRzOoPgb9fyQKHJo8zP6wDTdXGdJKjUvM7hTnIKVjzvy3xd2x1suW1INzWf__m4tr2xCtPvSKjZfvyQKa02VIZWO4x9bSu4eL2eQr724Ou74GAH7IoJvolpYLXi9jM9vEnt4TT0jfmlqoTUcF_RVZfinjPpihQ3obFoSJG21xfR1pH0EqTPRcBf6eff4Ia2ni-oaWfsCkkivBEyT_tjrq3bYkkPG7CFzFLma1njgBUoC26W7NEEzJheYoEzZtKlXD6YPE8UQ9feL0IU6zAp6sWS-XOeK1rVVUrD013wXyDkGMoLUZea-cNhk4L5j-wD890JXVSnZ6478oYnw_FowFbMtzzHnpy5I3c8nrjjkzPnZPw0YH81RV3boRh34njDoeedjJzxgCENlJA37Z8Mmot5tmBP_wHtB3Ud?type=png)](https://mermaid.live/edit#pako:eNqVVsFu20YQ_ZXpnmxUZEnKsiweXJB2ERupBcK0EaRVUWzIkUSY5LK7yzSq4d5yyye0P5cv6SxJhZIluYkOAld8M_Pe25ldPbJEpMh8pvCPGssELzO-kLyYlUAfXmtR1sU7lO26_a641FmSVbzUEABXcK9Q0tPRq6VQ-uesfDjeBU6Du98bMD0Y7K2oNco9wNiAXgmxyBHiu_vp_lThOlX4QqrwC7lwm1wLnQqNIN4b7oPQh2jJFYLrw3WKpc70Ci4zlZj3KzgyTI43TQis8_PvYx_eLAXwAq5_hKMwK9OsXMCtsVLpDh5bhLQCH96KGrhE8Jyh7diuO7Rdf-Q4DhxF9bs8S-A62qoQfluFsK_gTs7skWu7jmN7_ulOib3qjU0KElGtrIorenUdKSh4WfM8X8H7jLduyPSH-CZ-0UHPhzgr6lzzEkWt4ErQTkZ1mSyJ-nMLg7YyJMTuQcGMXYiyxETPWG9C-AKmReVCVPBTs1PkaKGM3rYexJpr7Izq-gMibPo10BqLSqv-5ZpV06wkA8uUysVvpzMGWkC4jWxQHdpwvIwg4skDamoXmfiGijOAS6V9MJtw_CzaeCezxVKDmENXsR2ORCKRViDrHH0I8lz82WQA67xJ-n-JiMznfz9145FKUSmS3RKbiiYrzIVxYIV6gxUntY014QFrwl7sc2uCXWvC3sgda057a0YHrMlxvimoFbPPmVHnzOl-ZzbzBL0xwTNjbnmC1P40XzoT5QYhEtoumoe9fW_bNgRzOoPgb9fyQKHJo8zP6wDTdXGdJKjUvM7hTnIKVjzvy3xd2x1suW1INzWf__m4tr2xCtPvSKjZfvyQKa02VIZWO4x9bSu4eL2eQr724Ou74GAH7IoJvolpYLXi9jM9vEnt4TT0jfmlqoTUcF_RVZfinjPpihQ3obFoSJG21xfR1pH0EqTPRcBf6eff4Ia2ni-oaWfsCkkivBEyT_tjrq3bYkkPG7CFzFLma1njgBUoC26W7NEEzJheYoEzZtKlXD6YPE8UQ9feL0IU6zAp6sWS-XOeK1rVVUrD013wXyDkGMoLUZea-cNhk4L5j-wD890JXVSnZ6478oYnw_FowFbMtzzHnpy5I3c8nrjjkzPnZPw0YH81RV3boRh34njDoeedjJzxgCENlJA37Z8Mmot5tmBP_wHtB3Ud)

1. **Initialization**: The application starts the Axum web server and the Tokio UDP listener simultaneously.
2. **Discovery**: The UDP layer queries a STUN server to resolve the machine's Public IP and punch a NAT hole.
3. **Signaling**: The user manually exchanges Public IPs with a peer via the Web UI.
4. **Transport**: Messages are routed from the UI → Shared State → KCP Stream → Peer.

---

## 🚀 Key Technical Features

- **True Peer-to-Peer**: Direct client-to-client connections minimize latency and remove dependency on third-party infrastructure.
- **Reliable UDP (ARQ)**: Utilizes KCP (via `tokio_kcp`) to provide TCP-like reliability with the speed advantages of UDP.
- **Automated NAT Traversal**: Integrated STUN client allows for seamless connectivity across different network configurations without manual port forwarding.
- **Asynchronous Core**: Built entirely on the Tokio runtime for non-blocking I/O and high concurrency.
- **Real-Time Updates**: State changes are pushed to the browser immediately via Server-Sent Events (SSE).

---

## 🛠️ Technology Stack

| Component     | Technology    | Role                                                 |
|---------------|---------------|-----------------------------------------------------|
| Runtime       | Tokio         | Asynchronous I/O scheduler and task management.     |
| Transport     | Tokio KCP     | Reliable UDP protocol implementation.               |
| Web Framework | Axum          | HTTP/REST interface and SSE stream handling.        |
| State         | Arc/RwLock    | Thread-safe state synchronization between tasks.    |
| Discovery     | STUN          | Public IP resolution and NAT hole punching.         |

---

## 🔒 Security Notice

> **WARNING**: *Protocol Status: Cleartext*

GhostLink v1.0 transmits data in plain text. While the transport layer provides reliability, it does not currently implement end-to-end encryption.

Do not transmit sensitive data (PII, credentials, financial information) over this version. Encryption is slated for the v1.1 release cycle (via QUIC).

---

## 📦 Installation & Usage

### Prerequisites

Ensure you have the latest stable version of Rust and Cargo installed.

### Quick Start

**Step 1**: Clone the repository.
```bash
git clone https://github.com/pushkar-gr/ghostlink.git
cd ghostlink
```

**Step 2**: Build and run.
```bash
cargo run --release
```

The first build may take a moment to compile dependencies.

**Step 3**: Initiate Connection.
- Navigate to `http://localhost:8080` in your web browser.
- Copy your Public IP displayed on the dashboard.
- Share your IP with a friend and input their IP into the Target Address field.
- Click **Establish Link**.

---

## 🤝 Contributing

We welcome contributions! 🚀

1. Fork the repository.
2. Create a feature branch.
   ```bash
   git checkout -b feature/amazing-feature
   ```
3. Commit your changes.
   ```bash
   git commit -m "Add some amazing feature"
   ```
4. Push to the branch.
   ```bash
   git push origin feature/amazing-feature
   ```
5. Open a Pull Request.

---

## 📄 License

This project is open-source and available under the **GNU General Public License v3.0**. See the [LICENSE](./LICENSE) file for details.

---

*Happy Chatting!* 👻
