use crate::Error;
use std::{
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
pub(super) type Stream = std::os::unix::net::UnixStream;

#[cfg(unix)]
pub(super) struct Connector;
#[cfg(unix)]
impl Connector {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self)
    }
    pub(super) fn connect(&self, path: &Path, _child: u32) -> io::Result<Stream> {
        Stream::connect(path)
    }
}

pub(super) fn endpoint(directory: &tempfile::TempDir) -> Result<PathBuf, Error> {
    #[cfg(unix)]
    {
        Ok(directory.path().join("ipc"))
    }
    #[cfg(windows)]
    {
        let name = directory
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(Error::InvalidInput)?;
        Ok(PathBuf::from(format!(r"\\.\pipe\{name}")))
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        cell::Cell,
        io::{Read, Write},
        os::windows::io::AsHandle,
        sync::Arc,
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::{ClientOptions, NamedPipeClient},
        runtime::{Builder, Runtime},
    };

    type MetadataPipe = interprocess::os::windows::named_pipe::DuplexPipeStream<
        interprocess::os::windows::named_pipe::pipe_mode::Bytes,
    >;

    pub(in super::super) struct Connector(Arc<Runtime>);
    impl Connector {
        pub(in super::super) fn new() -> io::Result<Self> {
            Ok(Self(Arc::new(
                Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()?,
            )))
        }
        pub(in super::super) fn connect(&self, path: &Path, child: u32) -> io::Result<Stream> {
            use windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION;
            // Local open returns immediately for absent/busy pipes, with explicit
            // identification-only SQOS even before checking the server's PID.
            let pipe = {
                let _guard = self.0.enter();
                ClientOptions::new()
                    .security_qos_flags(SECURITY_IDENTIFICATION)
                    .open(path)?
            };
            // Query via a duplicated handle and the synchronous safe wrapper only.
            // Never use interprocess 2.4.6's Tokio OwnedHandle conversion: its error
            // path returns ownership of a handle Tokio has already consumed.
            let metadata = MetadataPipe::try_from(pipe.as_handle().try_clone_to_owned()?)
                .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
            metadata.assume_flushed(); // No bytes were sent through this wrapper.
            let server = metadata.server_process_id()?;
            drop(metadata);
            if server != child {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            Ok(Stream {
                pipe,
                runtime: self.0.clone(),
                read_timeout: Cell::new(Duration::from_secs(3)),
                write_timeout: Cell::new(Duration::from_secs(3)),
            })
        }
    }

    /// Async pipe ownership stays inside this blocking worker interface. There is
    /// no helper thread, blocking FlushFileBuffers or detached pending operation.
    pub(in super::super) struct Stream {
        pipe: NamedPipeClient,
        runtime: Arc<Runtime>,
        read_timeout: Cell<Duration>,
        write_timeout: Cell<Duration>,
    }
    impl Stream {
        pub(in super::super) fn set_read_timeout(
            &self,
            timeout: Option<Duration>,
        ) -> io::Result<()> {
            self.read_timeout.set(finite_timeout(timeout)?);
            Ok(())
        }
        pub(in super::super) fn set_write_timeout(
            &self,
            timeout: Option<Duration>,
        ) -> io::Result<()> {
            self.write_timeout.set(finite_timeout(timeout)?);
            Ok(())
        }
    }
    fn finite_timeout(timeout: Option<Duration>) -> io::Result<Duration> {
        timeout
            .filter(|time| !time.is_zero())
            .ok_or(io::ErrorKind::InvalidInput.into())
    }
    impl Read for Stream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.runtime.block_on(async {
                tokio::time::timeout(self.read_timeout.get(), self.pipe.read(bytes))
                    .await
                    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
            })
        }
    }
    impl Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.runtime.block_on(async {
                tokio::time::timeout(self.write_timeout.get(), self.pipe.write(bytes))
                    .await
                    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
            })
        }
        fn flush(&mut self) -> io::Result<()> {
            // No userspace write buffer. Protocol replies provide acknowledgement.
            Ok(())
        }
    }
}

#[cfg(windows)]
pub(super) use windows::{Connector, Stream};

#[cfg(all(test, windows))]
#[path = "../../../../tests/native-spike/windows_transport.rs"]
mod tests;
