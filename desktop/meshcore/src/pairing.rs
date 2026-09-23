//! Pairing v1 (D013): the coordinator mints a one-time token and shows it in a QR together with
//! its address. The device presents the token in its first `Hello`; the coordinator answers with a
//! per-device random secret (`Paired`) that the device must present on every later `Hello`.
//! Tokens are single-use and expire; secrets are persisted by the coordinator (`state/paired.json`).
//! Transport encryption (Noise XX) remains D002's Phase-2 item; v1 relies on the private link.

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingOffer {
    pub mesh_id: String,
    pub host: String,
    pub control_port: u16,
    pub token: String, // hex, 16 bytes
}

impl PairingOffer {
    /// The payload encoded in the QR (compact JSON). Never serve this from a network endpoint.
    pub fn qr_payload(&self) -> String {
        serde_json::to_string(self).expect("serializable")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub device_id: String,
    pub name: String,
    pub secret_hex: String,
}

pub struct PairingBook {
    mesh_id: String,
    pending: HashMap<[u8; 32], Instant>,
    ttl: Duration,
    paired: HashMap<String, PairedDevice>,
}

/// Constant-time byte comparison (no early exit on the first mismatch).
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

impl PairingBook {
    pub fn new(mesh_id: impl Into<String>) -> Self {
        Self {
            mesh_id: mesh_id.into(),
            pending: HashMap::new(),
            ttl: Duration::from_secs(10 * 60),
            paired: HashMap::new(),
        }
    }

    pub fn mesh_id(&self) -> &str {
        &self.mesh_id
    }

    pub fn offer(&mut self, host: &str, control_port: u16) -> PairingOffer {
        let mut raw = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut raw);
        let token = hex::encode(raw);
        self.pending.insert(Self::digest(&raw), Instant::now());
        PairingOffer {
            mesh_id: self.mesh_id.clone(),
            host: host.to_string(),
            control_port,
            token,
        }
    }

    /// Consume a presented one-time token. On success the device is recorded and its new
    /// secret (32 random bytes) is returned; the caller sends it to the device once.
    pub fn redeem(&mut self, token_bytes: &[u8], device_id: &str, name: &str) -> Option<Vec<u8>> {
        let now = Instant::now();
        self.pending
            .retain(|_, t| now.duration_since(*t) < self.ttl);
        let raw: Vec<u8> = if token_bytes.len() == 32 {
            hex::decode(token_bytes).unwrap_or_default()
        } else {
            token_bytes.to_vec()
        };
        if raw.len() != 16 {
            return None;
        }
        let key = Self::digest(&raw);
        self.pending.remove(&key)?;
        let mut secret = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        self.paired.insert(
            device_id.to_string(),
            PairedDevice {
                device_id: device_id.to_string(),
                name: name.to_string(),
                secret_hex: hex::encode(&secret),
            },
        );
        Some(secret)
    }

    /// A reconnect must prove the secret issued at pairing.
    pub fn verify(&self, device_id: &str, secret: &[u8]) -> bool {
        match self.paired.get(device_id) {
            Some(p) => {
                let want = hex::decode(&p.secret_hex).unwrap_or_default();
                !secret.is_empty() && ct_eq(&want, secret)
            }
            None => false,
        }
    }

    pub fn is_paired(&self, device_id: &str) -> bool {
        self.paired.contains_key(device_id)
    }

    pub fn forget(&mut self, device_id: &str) {
        self.paired.remove(device_id);
    }

    pub fn export(&self) -> Vec<PairedDevice> {
        self.paired.values().cloned().collect()
    }

    pub fn import(&mut self, devices: Vec<PairedDevice>) {
        for d in devices {
            self.paired.insert(d.device_id.clone(), d);
        }
    }

    fn digest(raw: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(raw);
        h.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_single_use_and_secret_is_required_after() {
        let mut b = PairingBook::new("mesh-1");
        let o = b.offer("192.168.1.2", 7070);
        assert_eq!(o.token.len(), 32);
        let secret = b
            .redeem(o.token.as_bytes(), "phone-1", "POCO F5")
            .expect("first redeem");
        assert_eq!(secret.len(), 32);
        assert!(b.redeem(o.token.as_bytes(), "phone-2", "again").is_none());
        assert!(b.is_paired("phone-1"));
        assert!(b.verify("phone-1", &secret));
        assert!(!b.verify("phone-1", &[]), "empty secret must fail");
        assert!(!b.verify("phone-1", &[0u8; 32]), "wrong secret must fail");
        assert!(!b.verify("phone-9", &secret), "unknown device must fail");
        assert!(b
            .redeem(b"00000000000000000000000000000000", "phone-3", "bogus")
            .is_none());
    }

    #[test]
    fn export_import_roundtrip() {
        let mut b = PairingBook::new("m");
        let o = b.offer("h", 1);
        let s = b.redeem(o.token.as_bytes(), "d", "n").unwrap();
        let mut c = PairingBook::new("m");
        c.import(b.export());
        assert!(c.verify("d", &s));
    }

    #[test]
    fn ct_eq_basic() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }
}
