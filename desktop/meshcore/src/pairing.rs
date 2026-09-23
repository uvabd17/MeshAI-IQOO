//! Pairing v1: the coordinator mints a one-time token, shows it in a QR together with its
//! address; the device presents the token in its first `Hello`. Tokens are single-use and expire.
//! Transport encryption (Noise XX) is D002's Phase-2 item; v1 relies on the private link.

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
    /// The payload encoded in the QR (compact JSON).
    pub fn qr_payload(&self) -> String {
        serde_json::to_string(self).expect("serializable")
    }
}

pub struct PairingBook {
    mesh_id: String,
    pending: HashMap<[u8; 32], Instant>,
    ttl: Duration,
    paired: HashMap<String, String>, // device_id -> display name
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

    /// Consume a presented token. Returns true and records the device if it was a live one-time token.
    pub fn redeem(&mut self, token_bytes: &[u8], device_id: &str, name: &str) -> bool {
        let now = Instant::now();
        self.pending
            .retain(|_, t| now.duration_since(*t) < self.ttl);
        let raw: Vec<u8> = if token_bytes.len() == 32 {
            hex::decode(token_bytes).unwrap_or_default()
        } else {
            token_bytes.to_vec()
        };
        if raw.len() != 16 {
            return false;
        }
        let key = Self::digest(&raw);
        if self.pending.remove(&key).is_some() {
            self.paired.insert(device_id.to_string(), name.to_string());
            true
        } else {
            false
        }
    }

    pub fn is_paired(&self, device_id: &str) -> bool {
        self.paired.contains_key(device_id)
    }

    pub fn forget(&mut self, device_id: &str) {
        self.paired.remove(device_id);
    }

    pub fn paired(&self) -> impl Iterator<Item = (&String, &String)> {
        self.paired.iter()
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
    fn token_is_single_use() {
        let mut b = PairingBook::new("mesh-1");
        let o = b.offer("192.168.1.2", 7070);
        assert_eq!(o.token.len(), 32);
        assert!(b.redeem(o.token.as_bytes(), "phone-1", "POCO F5"));
        assert!(!b.redeem(o.token.as_bytes(), "phone-2", "again"));
        assert!(b.is_paired("phone-1"));
        assert!(!b.redeem(b"00000000000000000000000000000000", "phone-3", "bogus"));
    }
}
