//! Controller link for ChiXie writing/plotting machines: transports, Grbl streaming, the device
//! actor, ESP3D upload and LAN discovery. Protocol notes: `docs/protocol.md`.

pub mod device;
pub mod discovery;
pub mod esp3d;
pub mod grbl;
pub mod transport;
