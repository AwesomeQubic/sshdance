use std::path::Path;

use anyhow::Result;
use ed25519_dalek::SigningKey;
use russh_keys::key::KeyPair;
use tracing::info;

pub async fn get_or_create(path: impl AsRef<Path>) -> Result<KeyPair> {
    info!("Loading keypair");

    match tokio::fs::read(&path).await {
        Ok(key) => {
            let parsed = SigningKey::from_bytes(&key.try_into().expect("Invalid file"));

            Ok(KeyPair::Ed25519(parsed))
        }
        Err(err) => {
            info!("Encountered an error loading keypair {err} recreating it");
            let key = KeyPair::generate_ed25519();

            let KeyPair::Ed25519(ref inner) = key else {
                unreachable!()
            };

            tokio::fs::write(&path, inner.to_bytes()).await?;

            Ok(key)
        }
    }
}
