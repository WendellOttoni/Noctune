use anyhow::Result;

#[derive(Debug, Clone)]
pub struct NativeSpotifySession {
    is_active: bool,
    device_name: String,
}

impl NativeSpotifySession {
    pub fn new(device_name: String) -> Self {
        Self {
            is_active: false,
            device_name,
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn start(&mut self, _token: &str) -> Result<()> {
        self.is_active = false;
        anyhow::bail!(
            "Native Spotify audio is not available. Use an active Spotify Connect device."
        )
    }

    pub fn stop(&mut self) {
        self.is_active = false;
    }
}
