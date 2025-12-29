![License](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Build Status](https://img.shields.io/github/actions/workflow/status/pushkar-gr/ghostlink/ci.yml?label=Build)
![Made with Love](https://img.shields.io/badge/Made%20with-Rust-orange.svg)

---

# 👻 GhostLink

**Direct. Simple. Yours.**

Welcome to **GhostLink**! This is a serverless P2P chat application built with love (and a lot of Rust). It helps you connect directly with a friend without needing big central servers, complicated setups, or port forwarding headaches. It just works!

---

## ✨ What is it?

Imagine two tin cans connected by a string, but digital and engineered for speed. GhostLink lets you establish a direct link with another computer anywhere in the world.

* **No Intermediaries:** Your messages go straight to your friend.
* **Serverless:** We use a little magic (called STUN) to find each other, but the chat is all you.
* **Web Interface:** Comes with a clean, dark-mode UI that runs right in your browser.

## ⚠️ A Little Heads-Up (Important!)

While GhostLink is super cool for direct chats, please keep this in mind:

**There is NO Encryption.** 🔓

Think of this like passing a note in class or shouting across a playground. It's fast and direct, but it sends data as "plain text." Please **do not** send passwords, credit card numbers, or your deepest secrets over this link just yet. It's strictly for fun, casual chats!

## 🛠️ Under the Hood (For the Geeks)

We didn't just throw this together; we built it on a robust asynchronous stack. Here is the tech that makes the ghost fly:

* **Runtime:** Built on **Tokio** for asynchronous, non-blocking I/O, ensuring the app stays responsive even when juggling network packets.
* **Web Framework:** The UI is served by **Axum**, which also handles the REST API and pushes real-time updates to your browser via **Server-Sent Events (SSE)**.
* **Reliability:** We use **KCP** (via `tokio_kcp`) over UDP. This gives us the speed of UDP with the reliability of TCP (ARQ), meaning your messages won't get lost even if the network is a bit shaky.
* **NAT Traversal:** We implement **STUN** (Session Traversal Utilities for NAT) to punch holes through routers, allowing direct P2P connections without manual port forwarding.
* **State Management:** Thread-safe state sharing using `Arc<RwLock<AppState>>`, bridging the gap between the web server and the UDP network controller.

## 🚀 How to Run It

Getting started is easy if you have Rust installed.

1. **Clone the repo:**
```bash
git clone https://github.com/pushkar-gr/ghostlink.git
cd ghostlink

```


2. **Fire it up:**
```bash
cargo run

```


3. **Start Chatting:**
Open your browser and go to `http://localhost:8080`. Send your public IP to a friend, put their IP in the box, and hit **Establish Link**!

## 📄 License

This project is open-source and available under the **GNU General Public License v3.0**. Feel free to tinker, fork, and play around with it!

---

*Happy Chatting!* 👻
