use std::{net::SocketAddr, sync::Arc};

use client::ClientHandler;
use russh::{
    server::{Config, Server},
    MethodSet,
};
use russh_keys::key::KeyPair;
use site::SshPage;
use tracing::info;

mod client;
mod handle;
pub mod site;
pub mod util;

pub struct SshDanceBuilder {
    socket: SocketAddr,
    key_pair: Vec<KeyPair>,

    window_title: Option<&'static str>,

    initial_site: fn(Option<std::net::SocketAddr>) -> SshPage,
}

impl SshDanceBuilder {
    pub fn new(
        socket: SocketAddr,
        initial_site: fn(Option<std::net::SocketAddr>) -> SshPage,
    ) -> SshDanceBuilder {
        SshDanceBuilder {
            socket,
            key_pair: vec![KeyPair::generate_ed25519()],
            initial_site,
            window_title: None,
        }
    }

    pub fn set_window_title(mut self, title: &'static str) -> Self{
        self.window_title = Some(title);
        self
    }

    pub fn set_keys(mut self, key_pair: Vec<KeyPair>) -> Self {
        self.key_pair = key_pair;
        self
    }

    pub async fn run(self) -> Result<(), anyhow::Error> {
        let config = Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            methods: MethodSet::NONE,
            keys: self.key_pair,
            ..Default::default()
        };

        let mut server = SshSiteServer {
            initial_site: self.initial_site,
            window_title: self.window_title,
        };
        server.run(config, self.socket).await
    }
}

pub(crate) struct SshSiteServer {
    initial_site: fn(Option<std::net::SocketAddr>) -> SshPage,
    window_title: Option<&'static str>
}

impl SshSiteServer {
    async fn run(&mut self, config: Config, addr: SocketAddr) -> Result<(), anyhow::Error> {
        self.run_on_address(Arc::new(config), addr).await?;
        Ok(())
    }
}

impl Server for SshSiteServer {
    type Handler = ClientHandler;

    fn new_client(&mut self, addr: Option<std::net::SocketAddr>) -> Self::Handler {
        info!("New client connected {addr:?}");
        ClientHandler::new(addr, (self.initial_site)(addr), self.window_title.clone())
    }
}
