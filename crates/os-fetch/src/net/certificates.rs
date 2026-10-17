//! Zertifikatsprüfung und explizites Ermitteln einer Kette (keine automatische Übernahme).
use os_model::settings::NetworkSettings;
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{pem::PemObject, CertificateDer, ServerName, UnixTime},
    DigitallySignedStruct, SignatureScheme,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub fn roots(settings: &NetworkSettings) -> Result<Vec<CertificateDer<'static>>, String> {
    let mut roots = Vec::new();
    for path in &settings.trusted_certificates {
        let data =
            std::fs::read(path).map_err(|_| format!("Zertifikatsdatei nicht lesbar: {path}"))?;
        let certs = if data.starts_with(b"-----BEGIN") {
            CertificateDer::pem_slice_iter(&data)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| format!("Ungültiges PEM-Zertifikat: {path}"))?
        } else {
            vec![CertificateDer::from(data)]
        };
        if certs.is_empty() {
            return Err(format!("Keine Zertifikate in {path}"));
        }
        let mut check = rustls::RootCertStore::empty();
        for cert in certs {
            check
                .add(cert.clone())
                .map_err(|_| format!("Ungültiges Zertifikat: {path}"))?;
            roots.push(cert);
        }
    }
    Ok(roots)
}

#[derive(Debug, Clone, Default)]
pub struct CertificateChain {
    pub certificates: Vec<String>,
    pub trusted: bool,
}
#[derive(Debug)]
struct Capture {
    verifier: rustls_platform_verifier::Verifier,
    result: Arc<Mutex<CertificateChain>>,
}
impl ServerCertVerifier for Capture {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let trusted = self
            .verifier
            .verify_server_cert(end, intermediates, name, ocsp, now)
            .is_ok();
        *self.result.lock().unwrap_or_else(|p| p.into_inner()) = CertificateChain {
            certificates: std::iter::once(end)
                .chain(intermediates)
                .map(|c| pem(c.as_ref()))
                .collect(),
            trusted,
        };
        // Nur dieser ausdrückliche Probe-Client toleriert Zertifikatsfehler.
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        cert: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.verifier.verify_tls12_signature(m, cert, d)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        cert: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.verifier.verify_tls13_signature(m, cert, d)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.verifier.supported_verify_schemes()
    }
}
fn pem(der: &[u8]) -> String {
    let encoded = crate::auth::base64_encode(der);
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in encoded.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(line).unwrap());
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

pub async fn probe(
    url: &str,
    settings: &NetworkSettings,
    password: Option<&str>,
    resolve: &[String],
) -> Result<CertificateChain, String> {
    let url = url::Url::parse(url).map_err(|_| "Ungültige HTTPS-URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            "Zertifikate können nur für eine HTTPS-URL ohne Zugangsdaten ermittelt werden".into(),
        );
    }
    let roots = roots(settings)?;
    let result = Arc::new(Mutex::new(CertificateChain::default()));
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let verifier =
        rustls_platform_verifier::Verifier::new_with_extra_roots(roots, provider.clone())
            .map_err(|e| e.to_string())?;
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Capture {
            verifier,
            result: result.clone(),
        }))
        .with_no_client_auth();
    let resolver = crate::resolve::HostResolver::new(resolve, Duration::from_secs(10))?;
    let client = super::client_builder_with_password(settings, password)
        .tls_backend_preconfigured(config)
        .dns_resolver(Arc::new(resolver))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| crate::redact::text(&e.to_string()))?;
    // Der Handshake genügt; ein Server muss keine HTTP-Antwort liefern, damit die Kette sichtbar wird.
    let request = client.head(url).send().await;
    let chain = result.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if chain.certificates.is_empty() {
        return Err(request.err().map_or_else(
            || "Server lieferte kein Zertifikat".into(),
            |e| crate::redact::text(&e.to_string()),
        ));
    }
    Ok(chain)
}
