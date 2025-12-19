#[derive(Debug, Clone)]
pub struct Config {
    pub client_port: u16,
    pub stun_server: String,
    pub web_port: u16,
}

impl Config {
    pub fn load() -> Self {
        Self {
            client_port: 0,
            stun_server: "stun.l.google.com:19302".to_string(),
            web_port: 8080,
        }
    }
}
