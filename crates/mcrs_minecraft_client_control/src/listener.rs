use std::io;

#[cfg(unix)]
pub(crate) type Listener = std::os::unix::net::UnixListener;

#[cfg(unix)]
pub(crate) type Stream = std::os::unix::net::UnixStream;

#[cfg(not(unix))]
pub(crate) type Listener = std::net::TcpListener;

#[cfg(not(unix))]
pub(crate) type Stream = std::net::TcpStream;

#[cfg(unix)]
pub(crate) fn bind(address: &str) -> io::Result<Listener> {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::symlink_metadata(address) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            if Stream::connect(address).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another process serves the control socket",
                ));
            }
            std::fs::remove_file(address)?;
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the control path holds a file that is not a socket",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = Listener::bind(address)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

#[cfg(not(unix))]
pub(crate) fn bind(address: &str) -> io::Result<Listener> {
    let address: std::net::SocketAddr = address.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the control address is not ip:port",
        )
    })?;
    if !address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the control address is not a loopback address",
        ));
    }
    let listener = Listener::bind(address)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

#[cfg(unix)]
pub(crate) fn remove(address: &str) -> io::Result<()> {
    match std::fs::remove_file(address) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(not(unix))]
pub(crate) fn remove(_: &str) -> io::Result<()> {
    Ok(())
}
