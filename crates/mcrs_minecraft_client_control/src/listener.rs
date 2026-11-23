use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;

pub(crate) fn bind(address: &str) -> io::Result<UnixListener> {
    match fs::symlink_metadata(address) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            if UnixStream::connect(address).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another process serves the control socket",
                ));
            }
            fs::remove_file(address)?;
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
    let listener = UnixListener::bind(address)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

pub(crate) fn remove(address: &str) -> io::Result<()> {
    match fs::remove_file(address) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
