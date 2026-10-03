use thiserror::Error;

#[derive(Error, Debug)]
pub enum FluffyError {
    #[error("Wayland error: {0}")]
    Wayland(String),

    #[error("Wayland connection error: {0}")]
    WaylandConnect(#[from] wayland_client::ConnectError),

    #[error("Wayland global error: {0}")]
    WaylandGlobal(#[from] wayland_client::globals::GlobalError),

    #[error("Wayland dispatch error: {0}")]
    WaylandDispatch(#[from] wayland_client::DispatchError),

    #[error("GStreamer GLib error: {0}")]
    GstGlib(#[from] gstreamer::glib::Error),

    #[error("GStreamer GLib bool error: {0}")]
    GstBool(#[from] gstreamer::glib::BoolError),

    #[error("GStreamer state change error: {0}")]
    GstStateChange(#[from] gstreamer::StateChangeError),

    #[error("Playback error: {0}")]
    Playback(String),

    #[error("Output '{0}' not found")]
    OutputNotFound(String),

    #[error("Stale request generation: {requested} < current {current}")]
    StaleGeneration { current: u64, requested: u64 },

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("IPC JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Probe error: {0}")]
    Probe(String),

    #[error("Conversion error: {0}")]
    Conversion(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Job was cancelled")]
    JobCancelled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FluffyError>;
