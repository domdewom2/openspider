//! Test-CA und Host-Zertifikate für HTTPS auf demselben Port (F-TLS).
//!
//! Jede [`crate::TestSite`] besitzt eine eigene Test-CA (rcgen, ECDSA P-256), deren Zertifikat
//! über [`crate::TestSite::ca_cert_pem`] abrufbar ist. Zertifikate für Hostnamen entstehen bei
//! Bedarf anhand des SNI-Namens des Clients und werden zwischengespeichert:
//!
//! * `expired.<host>` (und jeder Name, dessen erstes Label `expired` ist) erhält ein
//!   **abgelaufenes** Zertifikat (gültig 2000-01-01 bis 2001-01-01),
//! * jeder andere Name erhält ein gültiges Zertifikat mit genau diesem Namen als SAN,
//! * ohne SNI (Verbindung per IP-Adresse) wird ein Zertifikat für `localhost`, `127.0.0.1`
//!   und `::1` ausgeliefert.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use rcgen::{
    date_time_ymd, BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose,
};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::{ClientConfig, RootCertStore, ServerConfig};

/// Erstes Label, das ein abgelaufenes Zertifikat auslöst (`expired.<host>`).
pub const EXPIRED_LABEL: &str = "expired";

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

fn tls_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("TLS-Testmaterial: {e}"))
}

/// Zertifikatsaussteller und Server-Konfiguration einer Testsite.
pub(crate) struct TlsMaterial {
    ca_pem: String,
    ca_der: CertificateDer<'static>,
    pub(crate) server_config: Arc<ServerConfig>,
}

impl TlsMaterial {
    pub(crate) fn new() -> io::Result<Self> {
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params
            .distinguished_name
            .push(DnType::CommonName, "OpenSpider Testsite CA");
        let key = KeyPair::generate().map_err(tls_err)?;
        let cert = params.self_signed(&key).map_err(tls_err)?;
        let ca_pem = cert.pem();
        let ca_der = cert.der().clone();
        let issuer = Issuer::new(params, key);

        let provider = provider();
        let resolver = Arc::new(HostCerts {
            issuer,
            provider: Arc::clone(&provider),
            cache: Mutex::new(HashMap::new()),
        });
        let mut server_config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(tls_err)?
            .with_no_client_auth()
            .with_cert_resolver(resolver);
        // HTTP/2 wird ausschließlich über TLS/ALPN ausgehandelt.
        server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(Self {
            ca_pem,
            ca_der,
            server_config: Arc::new(server_config),
        })
    }

    pub(crate) fn ca_pem(&self) -> &str {
        &self.ca_pem
    }

    pub(crate) fn ca_der(&self) -> &CertificateDer<'static> {
        &self.ca_der
    }
}

struct HostCerts {
    issuer: Issuer<'static, KeyPair>,
    provider: Arc<CryptoProvider>,
    cache: Mutex<HashMap<String, Arc<CertifiedKey>>>,
}

impl std::fmt::Debug for HostCerts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HostCerts")
    }
}

impl HostCerts {
    fn issue(&self, name: &str) -> io::Result<CertifiedKey> {
        let no_sni = name.is_empty();
        let sans: Vec<String> = if no_sni {
            vec!["localhost".into(), "127.0.0.1".into(), "::1".into()]
        } else {
            vec![if name.starts_with("wronghost.") {
                "other.test".to_string()
            } else {
                name.to_string()
            }]
        };
        let mut params = CertificateParams::new(sans).map_err(tls_err)?;
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params
            .distinguished_name
            .push(DnType::CommonName, if no_sni { "localhost" } else { name });
        if is_expired_name(name) {
            params.not_before = date_time_ymd(2000, 1, 1);
            params.not_after = date_time_ymd(2001, 1, 1);
        }
        let key = KeyPair::generate().map_err(tls_err)?;
        let cert = params.signed_by(&key, &self.issuer).map_err(tls_err)?;
        let der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let signing_key = self
            .provider
            .key_provider
            .load_private_key(der)
            .map_err(tls_err)?;
        Ok(CertifiedKey::new(vec![cert.der().clone()], signing_key))
    }
}

impl ResolvesServerCert for HostCerts {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let name = hello
            .server_name()
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(found) = cache.get(&name) {
            return Some(Arc::clone(found));
        }
        let issued = Arc::new(self.issue(&name).ok()?);
        cache.insert(name, Arc::clone(&issued));
        Some(issued)
    }
}

/// Ob für diesen SNI-Namen ein abgelaufenes Zertifikat ausgeliefert wird (`expired.<host>`).
pub fn is_expired_name(name: &str) -> bool {
    name.split('.').next() == Some(EXPIRED_LABEL) && name.contains('.')
}

/// Client-Konfiguration, die genau der Test-CA vertraut (`ca_pem` aus
/// [`crate::TestSite::ca_cert_pem`]); ALPN nur `http/1.1`. Für Tests mit eigenem TLS-Client.
pub fn client_config(ca_der: &CertificateDer<'static>) -> io::Result<Arc<ClientConfig>> {
    let mut roots = RootCertStore::empty();
    roots.add(ca_der.clone()).map_err(tls_err)?;
    let mut cfg = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_names() {
        assert!(is_expired_name("expired.site.test"));
        assert!(!is_expired_name("site.test"));
        assert!(!is_expired_name("expired"));
        assert!(!is_expired_name("notexpired.site.test"));
    }

    #[test]
    fn material_builds() {
        let m = TlsMaterial::new().unwrap();
        assert!(m.ca_pem().starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(!m.ca_der().is_empty());
    }
}
