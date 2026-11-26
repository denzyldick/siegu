//! Tiny loopback IPC used to keep a single desktop process alive.
//!
//! GPUI 0.3 does not expose cross-platform single-instance handling. This is a
//! std-only fallback: the first process binds a loopback port, and later
//! processes send their command line to it and wait for an acknowledgement
//! before exiting.

use std::{
    env, io,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

use siegu_core::config::APP_IDENTIFIER;

const MAX_FIELD: usize = 1024 * 1024;
const END: u32 = u32::MAX;

/// What happened when this process tried to claim the app port.
pub enum Instance {
    /// This process owns (or could not set up) the app port and should run.
    Primary(Option<SingleInstance>),
    /// Another Siegu process acknowledged the request; we should exit.
    Secondary,
}

/// The listening half of the single-instance handshake.
pub struct SingleInstance {
    listener: TcpListener,
}

fn port() -> u16 {
    let checksum: u32 = APP_IDENTIFIER.bytes().map(u32::from).sum();
    32_000 + (checksum % 20_000) as u16
}

/// Claim the app port, forwarding this launch to an existing process if one is
/// already listening.
pub fn claim() -> Instance {
    match TcpListener::bind(("127.0.0.1", port())) {
        Ok(listener) => Instance::Primary(Some(SingleInstance { listener })),
        Err(_) => {
            if notify(&current_request()) {
                Instance::Secondary
            } else {
                // A stale or hostile listener should not prevent the app from
                // starting. Single-instance is best-effort, not a hard lock.
                Instance::Primary(None)
            }
        }
    }
}

/// Command line plus working directory for the current launch.
pub fn current_request() -> Vec<String> {
    let mut request = Vec::new();
    request.push(
        env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    );
    request.extend(env::args().skip(1));
    request
}

/// Forward a launch request to the running instance. An acknowledgement keeps
/// an unrelated listener on the same port from silently swallowing the launch.
pub fn notify(request: &[String]) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port())) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if write_request(&mut stream, request).is_err() {
        return false;
    }
    let mut ack = [0_u8];
    matches!(stream.read_exact(&mut ack), Ok(()) if ack[0] == 1)
}

impl SingleInstance {
    /// Accept launch requests in a background thread and hand each one to
    /// `handler` as `{cwd, args...}`.
    pub fn serve(self, mut handler: impl FnMut(Vec<String>) + Send + 'static) {
        std::thread::Builder::new()
            .name("siegu-single-instance".to_string())
            .spawn(move || {
                for stream in self.listener.incoming().flatten() {
                    let mut stream = stream;
                    let request = read_request(&mut stream);
                    let ack = if request.is_ok() { 1 } else { 0 };
                    let _ = stream.write_all(&[ack]);
                    let _ = stream.flush();
                    if let Ok(request) = request {
                        handler(request);
                    }
                }
            })
            .expect("failed to start single-instance listener");
    }
}

fn write_request(stream: &mut TcpStream, request: &[String]) -> io::Result<()> {
    for field in request {
        let bytes = field.as_bytes();
        stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
        stream.write_all(bytes)?;
    }
    stream.write_all(&END.to_be_bytes())?;
    stream.flush()
}

fn read_request(stream: &mut TcpStream) -> io::Result<Vec<String>> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = Vec::new();
    loop {
        let mut length = [0_u8; 4];
        stream.read_exact(&mut length)?;
        let length = u32::from_be_bytes(length);
        if length == END {
            return Ok(request);
        }
        let length = length as usize;
        if length > MAX_FIELD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "field too large",
            ));
        }
        let mut bytes = vec![0_u8; length];
        stream.read_exact(&mut bytes)?;
        request.push(String::from_utf8_lossy(&bytes).into_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_is_stable() {
        assert_eq!(port(), port());
        assert!((32_000..52_000).contains(&port()));
    }

    #[test]
    fn request_round_trips_length_prefixed_fields() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("test listener");
        let address = listener.local_addr().expect("test address");
        let payload = vec![
            "/home/user/Pictures".to_string(),
            "/home/user/Pictures/sunset.jpg".to_string(),
        ];
        let expected = payload.clone();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("connect");
            write_request(&mut stream, &payload).expect("write");
            let mut ack = [0_u8];
            stream.read_exact(&mut ack).expect("ack");
            assert_eq!(ack, [1]);
        });
        let (mut stream, _) = listener.accept().expect("accept");
        assert_eq!(read_request(&mut stream).expect("read"), expected);
        stream.write_all(&[1]).expect("write ack");
        writer.join().expect("writer thread");
    }
}
