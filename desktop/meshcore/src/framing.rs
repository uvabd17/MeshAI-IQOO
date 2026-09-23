//! Length-prefixed protobuf frames: `u32 BE length` + `Envelope` bytes. Max 16 MiB.

use crate::proto::Envelope;
use prost::Message;

pub const MAX_FRAME: usize = 16 << 20;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame too large: {0}")]
    TooLarge(usize),
    #[error("decode: {0}")]
    Decode(#[from] prost::DecodeError),
}

pub fn encode(env: &Envelope) -> Vec<u8> {
    let body = env.encode_to_vec();
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// Try to pull one frame off the front of `buf`. Returns `None` if incomplete.
pub fn decode(buf: &mut Vec<u8>) -> Result<Option<Envelope>, FrameError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let env = Envelope::decode(&buf[4..4 + len])?;
    buf.drain(..4 + len);
    Ok(Some(env))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{envelope::Body, Heartbeat};

    #[test]
    fn roundtrip_and_partial() {
        let env = Envelope {
            version: 0,
            seq: 7,
            body: Some(Body::Heartbeat(Heartbeat { t_unix_ms: 123 })),
        };
        let bytes = encode(&env);
        let mut buf = bytes[..3].to_vec();
        assert!(decode(&mut buf).unwrap().is_none());
        buf.extend_from_slice(&bytes[3..]);
        let got = decode(&mut buf).unwrap().unwrap();
        assert_eq!(got.seq, 7);
        assert!(buf.is_empty());
    }
}
