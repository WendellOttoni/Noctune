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
        self.is_active = true;
        tracing::info!(target: "spotify_native", "Native Spotify PCM streaming module active for device '{}'", self.device_name);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_active = false;
    }
}
