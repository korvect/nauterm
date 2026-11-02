use std::io::{self, ErrorKind};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use polling::{Event, Events, Poller};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

const TELNET_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TELNET_CONNECT_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn connect_tcp(
    host: &str,
    port: u16,
    shutdown: &AtomicBool,
) -> Result<TcpStream, String> {
    let mut last_error = None;
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve {host}:{port}: {error}"))?;

    for address in addresses {
        if shutdown.load(Ordering::Acquire) {
            return Err("connection cancelled".to_owned());
        }
        match connect_tcp_address(address, shutdown) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }

    let message = last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "no resolved addresses".to_owned());
    Err(format!("failed to connect to {host}:{port}: {message}"))
}

fn connect_tcp_address(address: SocketAddr, shutdown: &AtomicBool) -> io::Result<TcpStream> {
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(Protocol::TCP),
    )?;
    socket.set_nonblocking(true)?;

    match socket.connect(&SockAddr::from(address)) {
        Ok(()) => return Ok(socket.into()),
        Err(error) if is_connect_in_progress(&error) => {}
        Err(error) => return Err(error),
    }

    let poller = Poller::new()?;
    unsafe {
        poller.add(&socket, Event::writable(0))?;
    }

    let deadline = Instant::now() + TELNET_CONNECT_TIMEOUT;
    let mut events = Events::new();
    loop {
        if shutdown.load(Ordering::Acquire) {
            let _ = poller.delete(&socket);
            return Err(io::Error::new(
                ErrorKind::Interrupted,
                "connection cancelled",
            ));
        }

        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            let _ = poller.delete(&socket);
            return Err(ErrorKind::TimedOut.into());
        };
        let wait = remaining.min(TELNET_CONNECT_POLL_INTERVAL);
        events.clear();

        match poller.wait(&mut events, Some(wait)) {
            Ok(0) => continue,
            Ok(_) => {
                let _ = poller.delete(&socket);
                if let Some(error) = socket.take_error()? {
                    return Err(error);
                }
                return Ok(socket.into());
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = poller.delete(&socket);
                return Err(error);
            }
        }
    }
}

fn is_connect_in_progress(error: &io::Error) -> bool {
    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) {
        return true;
    }
    let raw = error.raw_os_error();
    #[cfg(unix)]
    {
        if raw == Some(libc::EINPROGRESS) || raw == Some(libc::EALREADY) {
            return true;
        }
    }
    #[cfg(windows)]
    {
        if raw == Some(10035) || raw == Some(10036) {
            return true;
        }
    }
    false
}

pub(super) fn telnet_runtime_error(operation: &str, error: &io::Error) -> String {
    match error.kind() {
        ErrorKind::PermissionDenied => {
            format!("Permission denied while trying to {operation} the telnet socket.")
        }
        ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof
        | ErrorKind::ConnectionAborted
        | ErrorKind::ConnectionReset => {
            format!("Telnet connection closed while trying to {operation} it.")
        }
        _ => format!("Telnet I/O error while trying to {operation} the socket: {error}"),
    }
}

pub(super) fn is_transient_io(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock)
}
