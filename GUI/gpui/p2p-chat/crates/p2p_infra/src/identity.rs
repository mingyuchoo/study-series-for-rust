use crate::{protocol::fingerprint,
            quic::SERVER_NAME};
use rustls::pki_types::{CertificateDer,
                        PrivateKeyDer,
                        PrivatePkcs8KeyDer};
use serde::{Deserialize,
            Serialize};
use std::{collections::BTreeMap,
          fs,
          io::{self,
               Write},
          net::SocketAddr,
          path::{Path,
                 PathBuf}};

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    certificate: Vec<u8>,
    private_key: Vec<u8>,
}

pub(crate) struct Identity {
    pub certificate: CertificateDer<'static>,
    pub key: PrivateKeyDer<'static>,
    pub node_id: String,
}

impl Identity {
    pub fn load(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let path = directory.join("identity.json");
        let stored: StoredIdentity = match fs::read(&path) {
            | Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            | Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let generated = rcgen::generate_simple_self_signed(vec![SERVER_NAME.into()]).map_err(io::Error::other)?;
                let stored = StoredIdentity {
                    certificate: generated.cert.der().to_vec(),
                    private_key: generated.signing_key.serialize_der(),
                };
                write_json(&path, &stored)?;
                stored
            },
            | Err(error) => return Err(error),
        };
        Ok(Self {
            node_id: fingerprint(&stored.certificate),
            certificate: CertificateDer::from(stored.certificate),
            key: PrivatePkcs8KeyDer::from(stored.private_key).into(),
        })
    }
}

pub(crate) struct TrustStore {
    path: PathBuf,
    pins: BTreeMap<String, String>,
}

impl TrustStore {
    pub fn load(directory: &Path) -> io::Result<Self> {
        let path = directory.join("trusted-peers.json");
        let pins = match fs::read(&path) {
            | Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            | Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            | Err(error) => return Err(error),
        };
        Ok(Self {
            path,
            pins,
        })
    }

    pub fn accepts(&self, address: SocketAddr, node_id: &str) -> bool { self.pins.get(&address.to_string()).is_none_or(|pin| pin == node_id) }

    pub fn pin(&mut self, address: SocketAddr, node_id: &str) -> io::Result<()> {
        if !self.accepts(address, node_id) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "peer certificate changed"));
        }
        let address = address.to_string();
        if self.pins.contains_key(&address) {
            return Ok(());
        }
        self.pins.insert(address.clone(), node_id.into());
        if let Err(error) = write_json(&self.path, &self.pins) {
            self.pins.remove(&address);
            return Err(error);
        }
        Ok(())
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec(value).map_err(io::Error::other)?)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}
