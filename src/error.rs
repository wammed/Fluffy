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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FluffyError>;
