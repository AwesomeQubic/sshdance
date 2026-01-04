use std::{marker::PhantomData, net::SocketAddr, sync::Arc};

use russh::{
    keys::PrivateKey,
    server::{Config, Server},
    MethodKind, MethodSet,
};
use tracing::info;

pub mod api;
mod error;
mod internal;
pub mod util;

pub use error::Error;

use crate::{api::ClientHandler, internal::SshSessionHandler};

pub struct SshDanceBuilder<H: ClientHandler> {
    socket: SocketAddr,
    key_pair: Vec<PrivateKey>,

    data: PhantomData<H>,
}

impl<H: ClientHandler> SshDanceBuilder<H> {
    pub fn new(socket: SocketAddr) -> Self {
        Self {
            socket,
            key_pair: vec![PrivateKey::random(
                &mut rand_core::OsRng,
                russh::keys::Algorithm::Ed25519,
            )
            .unwrap()],
            data: PhantomData,
        }
    }

    pub fn set_keys(mut self, key_pair: Vec<PrivateKey>) -> Self {
        self.key_pair = key_pair;
        self
    }

    pub async fn run(self) -> Result<(), crate::Error> {
        let config = Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            methods: {
                let mut set = MethodSet::empty();
                set.push(MethodKind::None);
                set
            },
            keys: self.key_pair,
            ..Default::default()
        };

        let mut server: SshSiteServer<H> = SshSiteServer { data: PhantomData };
        server.run(config, self.socket).await
    }
}

pub(crate) struct SshSiteServer<H: ClientHandler> {
    data: PhantomData<H>,
}

impl<H: ClientHandler> SshSiteServer<H> {
    async fn run(&mut self, config: Config, addr: SocketAddr) -> Result<(), crate::Error> {
        self.run_on_address(Arc::new(config), addr).await?;
        Ok(())
    }
}

impl<H: ClientHandler> Server for SshSiteServer<H> {
    type Handler = SshSessionHandler<H>;

    fn new_client(&mut self, addr: Option<std::net::SocketAddr>) -> Self::Handler {
        info!("New client connected {addr:?}");
        SshSessionHandler::create(addr)
    }
}
