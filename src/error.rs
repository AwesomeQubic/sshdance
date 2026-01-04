use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Data sent to unknown channel")]
    UnknownChannel,

    #[error("Got PTY request before session open request")]
    PtyRequestBeforeOpenRequest,

    #[error("Got PTY twice")]
    PtyRequestTwice,

    #[error("Session closed")]
    SessionClosed,

    #[error("Enocuntered russh error {0}")]
    RusshError(#[from] russh::Error),

    #[error("Io error")]
    IoError(#[from] std::io::Error),

    #[error("Enocuntered russh key error {0}")]
    RusshKeyError(#[from] russh::keys::Error)
}
