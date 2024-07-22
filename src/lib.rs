pub const BUFFER_SIZE : (&str, &str, &str, &str)= ("BufferSize", "--buffer-size", "-b", "Maximum buffer size");

mod base;
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    str::FromStr,
};

pub use base::base::{
    BoxedClone, DebugLevel, Entry, EntryStatic, Error, Pipeline, Step, StepStatic,
};

mod stdio;
pub use stdio::stdio::{StdioEntry, StdioStep};

mod tcp;
// pub use tcp::tcp;

pub fn create_socket_addr(address: &str, port: u16) -> Result<SocketAddr, Error> {
    if let Ok(ip) = IpAddr::from_str(address) {
        return Ok(SocketAddr::new(ip, port));
    }

    let socket_addrs = format!("{}:{}", address, port);
    match socket_addrs.to_socket_addrs() {
        Ok(mut iter) => iter
            .next()
            .ok_or_else(|| Error::Msg("No addresses found".to_string())),
        Err(e) => Err(Error::Msg(format!("Failed to resolve address: {}", e))),
    }
}
