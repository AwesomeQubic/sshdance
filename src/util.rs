use std::path::Path;

use russh::keys::PrivateKey;
use tracing::info;

pub async fn get_or_create(path: impl AsRef<Path>) -> Result<PrivateKey, crate::Error> {
    info!("Loading keypair");
    let path: &Path = path.as_ref();

    match PrivateKey::read_openssh_file(path) {
        Ok(key) => Ok(key),
        Err(err) => {
            info!("Encountered an error loading keypair {err} recreating it");
            let key = PrivateKey::random(&mut rand_core::OsRng, russh::keys::Algorithm::Ed25519)
                .expect("Unable to create the key");

            let _ = key.write_openssh_file(path, russh::keys::ssh_key::LineEnding::default());

            Ok(key)
        }
    }
}
