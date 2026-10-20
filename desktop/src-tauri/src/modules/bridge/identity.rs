//! The PC's bridge identity (Architecture.md §4.1): a stable id plus a self-signed TLS
//! certificate. The phone pins the certificate's SHA-256 fingerprint from the pairing QR, so no
//! certificate authority is involved. The private key lives in the OS keychain, never in SQLite.

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::foundation::{
    errors::{PrekuError, Result},
    secrets,
    storage::Storage,
};

const SECRET_KEY: &str = "bridge.identity";
const SETTINGS_KEY: &str = "bridge.identity";

#[derive(Clone)]
pub struct Identity {
    pub pc_id: String,
    pub cert_der: Vec<u8>,
    key_der: Vec<u8>,
}

impl std::fmt::Debug for Identity {
    // Never print the private key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("pc_id", &self.pc_id)
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct StoredKey {
    cert_pem: String,
    key_pem: String,
}

#[derive(Serialize, Deserialize)]
struct StoredId {
    pc_id: String,
}

/// SHA-256 of a DER certificate, as lowercase hex (what the QR carries and what gets pinned).
pub fn fingerprint(cert_der: &[u8]) -> String {
    Sha256::digest(cert_der)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Identity {
    /// A fresh identity (ECDSA P-256: widely supported by Android's TLS stack).
    pub fn generate(pc_id: String) -> Result<Self> {
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(crypto)?;
        let mut params = CertificateParams::new(vec!["preku.local".to_owned()]).map_err(crypto)?;
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, format!("Preku {pc_id}"));
        params.distinguished_name = name;
        // Long-lived: the phone pins the exact certificate, so expiry adds nothing but breakage.
        params.not_before = rcgen::date_time_ymd(2025, 1, 1);
        params.not_after = rcgen::date_time_ymd(2125, 1, 1);
        let cert = params.self_signed(&key).map_err(crypto)?;
        Ok(Self {
            pc_id,
            cert_der: cert.der().to_vec(),
            key_der: key.serialize_der(),
        })
    }

    /// Loads the identity, creating (and storing) it on first use.
    pub fn load_or_create(storage: &Storage) -> Result<Self> {
        let pc_id = match storage.settings_get(SETTINGS_KEY)? {
            Some(value) => serde_json::from_value::<StoredId>(value)?.pc_id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                storage.settings_set(
                    SETTINGS_KEY,
                    &serde_json::to_value(StoredId { pc_id: id.clone() })?,
                )?;
                id
            }
        };
        if let Some(raw) = secrets::get(SECRET_KEY)? {
            let stored: StoredKey = serde_json::from_str(&raw)?;
            let key = KeyPair::from_pem(&stored.key_pem).map_err(crypto)?;
            let cert = CertificateDer::from_pem_slice(stored.cert_pem.as_bytes())
                .map_err(|e| PrekuError::Bridge(format!("bad stored certificate: {e}")))?;
            return Ok(Self {
                pc_id,
                cert_der: cert.to_vec(),
                key_der: key.serialize_der(),
            });
        }
        let identity = Self::generate(pc_id)?;
        let key = KeyPair::try_from(identity.key_der.as_slice()).map_err(crypto)?;
        let stored = StoredKey {
            cert_pem: pem("CERTIFICATE", &identity.cert_der),
            key_pem: key.serialize_pem(),
        };
        secrets::set(SECRET_KEY, &serde_json::to_string(&stored)?)?;
        Ok(identity)
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.cert_der)
    }

    pub fn cert(&self) -> CertificateDer<'static> {
        CertificateDer::from(self.cert_der.clone())
    }

    pub fn key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone()))
    }
}

fn crypto(err: rcgen::Error) -> PrekuError {
    PrekuError::Bridge(format!("certificate: {err}"))
}

fn pem(label: &str, der: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identity_has_a_stable_fingerprint() {
        let a = Identity::generate("pc-1".into()).unwrap();
        assert_eq!(a.fingerprint(), fingerprint(&a.cert_der));
        assert_eq!(a.fingerprint().len(), 64);
        let b = Identity::generate("pc-1".into()).unwrap();
        assert_ne!(
            a.fingerprint(),
            b.fingerprint(),
            "a new key is a new identity"
        );
        assert!(
            !format!("{a:?}").contains("key_der"),
            "debug never prints the key"
        );
    }

    #[test]
    fn pem_round_trips_through_rustls() {
        let id = Identity::generate("pc-1".into()).unwrap();
        let text = pem("CERTIFICATE", &id.cert_der);
        let back = CertificateDer::from_pem_slice(text.as_bytes()).unwrap();
        assert_eq!(back.as_ref(), id.cert_der.as_slice());
    }
}
