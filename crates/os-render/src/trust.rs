//! Explizite Grenze bis zur Freigabe der Netzwerkbrücke durch M2-N01.
//! Keine TLS-Prüfung wird abgeschaltet und keine konfigurierte CA still ignoriert.
use super::{LaunchOpts, RenderError};
use os_model::{ProxyKind, ProxyMode};

pub fn validate(launch: &LaunchOpts) -> Result<(), RenderError> {
    if !launch.network.trusted_certificates.is_empty() {
        return Err(RenderError::NotImplemented(
            "M2-N01: zusätzliche Truststore-Zertifikate benötigen die Netzwerkbrücke",
        ));
    }
    if launch.network.proxy.mode == ProxyMode::Manual
        && launch.network.proxy.kind == ProxyKind::Socks5
        && launch.proxy_credentials.is_some()
    {
        return Err(RenderError::NotImplemented(
            "M2-N01: authentifiziertes SOCKS5 benötigt die Netzwerkbrücke",
        ));
    }
    Ok(())
}
