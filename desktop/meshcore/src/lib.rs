//! meshcore — the part of MeshAI that decides things.
//!
//! * [`gguf`]     read a GGUF file's metadata and per-layer byte sizes without loading it
//! * [`planner`]  place a model across devices (host + workers) with a reason for every choice
//! * [`proto`]    the control-plane messages (generated from `proto/mesh.proto`)
//! * [`framing`]  length-prefixed protobuf frames for the control stream
//! * [`pairing`]  one-time token pairing (v1: token proof on a private link)

pub mod framing;
pub mod gguf;
pub mod pairing;
pub mod planner;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/meshai.v0.rs"));
}

pub const CONTROL_PORT: u16 = 7070;
pub const RPC_PORT: u16 = 50052;
pub const HOST_LLAMA_PORT: u16 = 8081;
pub const API_PORT: u16 = 8080;
