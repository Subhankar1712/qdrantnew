use std::path::Path;
use std::{fs, io, result};

use storage::content_manager::errors::StorageError;

use crate::settings::{Settings, TlsConfig};

#[derive(Clone)]
pub struct HttpClient {
    tls_config: Option<TlsConfig>,
    verify_https_client_certificate: bool,
}

impl HttpClient {
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        let tls_config = if settings.service.enable_tls {
            let Some(tls_config) = settings.tls.clone() else {
                return Err(Error::TlsConfigUndefined);
            };

            Some(tls_config)
        } else {
            None
        };

        let verify_https_client_certificate = settings.service.verify_https_client_certificate;

        let http_client = Self {
            tls_config,
            verify_https_client_certificate,
        };

        Ok(http_client)
    }

    pub fn client(&self) -> Result<reqwest::Client> {
        match &self.tls_config {
            Some(tls_config) => https_client(tls_config, self.verify_https_client_certificate),
            None => Ok(reqwest::Client::new()),
        }
    }
}

fn https_client(
    tls_config: &TlsConfig,
    verify_https_client_certificate: bool,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .add_root_certificate(https_client_ca_cert(tls_config.ca_cert.as_ref())?);

    if verify_https_client_certificate {
        builder = builder.identity(https_client_identity(
            tls_config.cert.as_ref(),
            tls_config.key.as_ref(),
        )?);
    }

    let client = builder.build()?;

    Ok(client)
}

fn https_client_ca_cert(ca_cert: &Path) -> Result<reqwest::tls::Certificate> {
    let ca_cert_pem =
        fs::read(ca_cert).map_err(|err| Error::failed_to_read(err, "CA certificate", ca_cert))?;

    let ca_cert = reqwest::Certificate::from_pem(&ca_cert_pem)?;

    Ok(ca_cert)
}

fn https_client_identity(cert: &Path, key: &Path) -> Result<reqwest::tls::Identity> {
    let mut identity_pem =
        fs::read(cert).map_err(|err| Error::failed_to_read(err, "certificate", cert))?;

    let mut key_file = fs::File::open(key).map_err(|err| Error::failed_to_read(err, "key", key))?;

    io::copy(&mut key_file, &mut identity_pem)
        .map_err(|err| Error::failed_to_read(err, "key", key))?;

    let identity = reqwest::Identity::from_pem(&identity_pem)?;

    Ok(identity)
}

pub type Result<T, E = Error> = result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("TLS config is not defined in the Qdrant config file")]
    TlsConfigUndefined,

    #[error("{1}: {0}")]
    Io(#[source] io::Error, String),

    #[error("failed to setup HTTPS client: {0}")]
    Reqwest(#[from] reqwest::Error),
}

impl Error {
    pub fn io(source: io::Error, context: impl Into<String>) -> Self {
        Self::Io(source, context.into())
    }

    pub fn failed_to_read(source: io::Error, file: &str, path: &Path) -> Self {
        Self::io(
            source,
            format!("failed to read HTTPS client {file} file {}", path.display()),
        )
    }
}

impl From<Error> for StorageError {
    fn from(err: Error) -> Self {
        StorageError::service_error(format!("failed to initialize HTTP(S) client: {err}"))
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}
