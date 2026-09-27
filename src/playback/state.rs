#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Paused,
    Playing,
}

impl std::fmt::Display for PlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackState::Stopped => write!(f, "STOPPED"),
            PlaybackState::Paused => write!(f, "PAUSED"),
            PlaybackState::Playing => write!(f, "PLAYING"),
        }
    }
}
