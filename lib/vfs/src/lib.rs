use std::any::Any;
use std::ffi::OsString;
use std::fmt;
use std::future::Future;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Waker, Context, Poll};
use thiserror::Error;

#[cfg(all(not(feature = "host-fs"), not(feature = "mem-fs")))]
compile_error!("At least the `host-fs` or the `mem-fs` feature must be enabled. Please, pick one.");

//#[cfg(all(feature = "mem-fs", feature = "enable-serde"))]
//compile_warn!("`mem-fs` does not support `enable-serde` for the moment.");

#[cfg(feature = "host-fs")]
pub mod host_fs;
#[cfg(feature = "mem-fs")]
pub mod mem_fs;
#[cfg(feature = "static-fs")]
pub mod static_fs;
#[cfg(feature = "webc-fs")]
pub mod webc_fs;

pub type Result<T> = std::result::Result<T, FsError>;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct FileDescriptor(usize);

impl From<u32> for FileDescriptor {
    fn from(a: u32) -> Self {
        Self(a as usize)
    }
}

impl From<FileDescriptor> for u32 {
    fn from(a: FileDescriptor) -> u32 {
        a.0 as u32
    }
}

pub trait FileSystem: fmt::Debug + Send + Sync + 'static + Upcastable {
    fn read_dir(&self, path: &Path) -> Result<ReadDir>;
    fn create_dir(&self, path: &Path) -> Result<()>;
    fn remove_dir(&self, path: &Path) -> Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<Metadata>;
    /// This method gets metadata without following symlinks in the path.
    /// Currently identical to `metadata` because symlinks aren't implemented
    /// yet.
    fn symlink_metadata(&self, path: &Path) -> Result<Metadata> {
        self.metadata(path)
    }
    fn remove_file(&self, path: &Path) -> Result<()>;

    fn new_open_options(&self) -> OpenOptions;
}

impl dyn FileSystem + 'static {
    #[inline]
    pub fn downcast_ref<T: 'static>(&'_ self) -> Option<&'_ T> {
        self.upcast_any_ref().downcast_ref::<T>()
    }
    #[inline]
    pub fn downcast_mut<T: 'static>(&'_ mut self) -> Option<&'_ mut T> {
        self.upcast_any_mut().downcast_mut::<T>()
    }
}

pub trait FileOpener {
    fn open(
        &mut self,
        path: &Path,
        conf: &OpenOptionsConfig,
    ) -> Result<Box<dyn VirtualFile + Send + Sync + 'static>>;
}

#[derive(Debug, Clone)]
pub struct OpenOptionsConfig {
    pub read: bool,
    pub write: bool,
    pub create_new: bool,
    pub create: bool,
    pub append: bool,
    pub truncate: bool,
}

impl OpenOptionsConfig {
    /// Returns the minimum allowed rights, given the rights of the parent directory
    pub fn minimum_rights(&self, parent_rights: &Self) -> Self {
        Self {
            read: parent_rights.read && self.read,
            write: parent_rights.write && self.write,
            create_new: parent_rights.create_new && self.create_new,
            create: parent_rights.create && self.create,
            append: parent_rights.append && self.append,
            truncate: parent_rights.truncate && self.truncate,
        }
    }

    pub const fn read(&self) -> bool {
        self.read
    }

    pub const fn write(&self) -> bool {
        self.write
    }

    pub const fn create_new(&self) -> bool {
        self.create_new
    }

    pub const fn create(&self) -> bool {
        self.create
    }

    pub const fn append(&self) -> bool {
        self.append
    }

    pub const fn truncate(&self) -> bool {
        self.truncate
    }
}

impl fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.conf.fmt(f)
    }
}

pub struct OpenOptions {
    opener: Box<dyn FileOpener>,
    conf: OpenOptionsConfig,
}

impl OpenOptions {
    pub fn new(opener: Box<dyn FileOpener>) -> Self {
        Self {
            opener,
            conf: OpenOptionsConfig {
                read: false,
                write: false,
                create_new: false,
                create: false,
                append: false,
                truncate: false,
            },
        }
    }

    pub fn get_config(&self) -> OpenOptionsConfig {
        self.conf.clone()
    }

    pub fn options(&mut self, options: OpenOptionsConfig) -> &mut Self {
        self.conf = options;
        self
    }

    pub fn read(&mut self, read: bool) -> &mut Self {
        self.conf.read = read;
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.conf.write = write;
        self
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        self.conf.append = append;
        self
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.conf.truncate = truncate;
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.conf.create = create;
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.conf.create_new = create_new;
        self
    }

    pub fn open<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<Box<dyn VirtualFile + Send + Sync + 'static>> {
        self.opener.open(path.as_ref(), &self.conf)
    }
}

/// This trait relies on your file closing when it goes out of scope via `Drop`
//#[cfg_attr(feature = "enable-serde", typetag::serde)]
#[async_trait::async_trait]
pub trait VirtualFile: fmt::Debug + Write + Read + Seek + Upcastable {
    /// the last time the file was accessed in nanoseconds as a UNIX timestamp
    fn last_accessed(&self) -> u64;

    /// the last time the file was modified in nanoseconds as a UNIX timestamp
    fn last_modified(&self) -> u64;

    /// the time at which the file was created in nanoseconds as a UNIX timestamp
    fn created_time(&self) -> u64;

    /// the size of the file in bytes
    fn size(&self) -> u64;

    /// Change the size of the file, if the `new_size` is greater than the current size
    /// the extra bytes will be allocated and zeroed
    fn set_len(&mut self, new_size: u64) -> Result<()>;

    /// Request deletion of the file
    fn unlink(&mut self) -> Result<()>;

    /// Store file contents and metadata to disk
    /// Default implementation returns `Ok(())`.  You should implement this method if you care
    /// about flushing your cache to permanent storage
    fn sync_to_disk(&self) -> Result<()> {
        Ok(())
    }

    /// Returns the number of bytes available.  This function must not block
    fn bytes_available(&self) -> Result<usize> {
        Ok(self.bytes_available_read()?.unwrap_or(0usize)
            + self.bytes_available_write()?.unwrap_or(0usize))
    }

    /// Returns the number of bytes available.  This function must not block
    /// Defaults to `None` which means the number of bytes is unknown
    fn bytes_available_read(&self) -> Result<Option<usize>> {
        Ok(None)
    }

    /// Returns the number of bytes available.  This function must not block
    /// Defaults to `None` which means the number of bytes is unknown
    fn bytes_available_write(&self) -> Result<Option<usize>> {
        Ok(None)
    }

    /// Polls for when read data is available again
    /// Defaults to `None` which means no asynchronous IO support - caller
    /// must poll `bytes_available_read` instead
    fn poll_read_ready(
        &self,
        cx: &mut std::task::Context<'_>,
        register_root_waker: &Arc<dyn Fn(Waker) + Send + Sync + 'static>,
    ) -> std::task::Poll<Result<usize>> {
        use std::ops::Deref;
        match self.bytes_available_read() {
            Ok(Some(0)) => {
                let waker = cx.waker().clone();
                register_root_waker.deref()(waker);
                std::task::Poll::Pending
            }
            Ok(Some(a)) => std::task::Poll::Ready(Ok(a)),
            Ok(None) => std::task::Poll::Ready(Err(FsError::WouldBlock)),
            Err(err) => std::task::Poll::Ready(Err(err)),
        }
    }

    /// Polls for when the file can be written to again
    /// Defaults to `None` which means no asynchronous IO support - caller
    /// must poll `bytes_available_write` instead
    fn poll_write_ready(
        &self,
        cx: &mut std::task::Context<'_>,
        register_root_waker: &Arc<dyn Fn(Waker) + Send + Sync + 'static>,
    ) -> std::task::Poll<Result<usize>> {
        use std::ops::Deref;
        match self.bytes_available_write() {
            Ok(Some(0)) => {
                let waker = cx.waker().clone();
                register_root_waker.deref()(waker);
                std::task::Poll::Pending
            }
            Ok(Some(a)) => std::task::Poll::Ready(Ok(a)),
            Ok(None) => std::task::Poll::Ready(Err(FsError::WouldBlock)),
            Err(err) => std::task::Poll::Ready(Err(err)),
        }
    }

    /// Polls for when the file can be written to again
    /// Defaults to `None` which means no asynchronous IO support - caller
    /// must poll `bytes_available_write` instead
    fn poll_close_ready(
        &self,
        cx: &mut std::task::Context<'_>,
        register_root_waker: &Arc<dyn Fn(Waker) + Send + Sync + 'static>,
    ) -> std::task::Poll<()> {
        use std::ops::Deref;
        match self.is_open() {
            true => {
                let waker = cx.waker().clone();
                register_root_waker.deref()(waker);
                std::task::Poll::Pending
            }
            false => std::task::Poll::Ready(()),
        }
    }

    /// Asynchronously reads from this file
    fn read_async<'a>(&'a mut self, max_size: usize, register_root_waker: &'_ Arc<dyn Fn(Waker) + Send + Sync + 'static>) -> Box<dyn Future<Output=io::Result<Vec<u8>>> + 'a>
    where Self: Sized
    {
        Box::new(VirtualFileAsyncRead {
            file: self,
            buf: Some(Vec::with_capacity(max_size)),
            register_root_waker: register_root_waker.clone()
        })
    }

    /// Asynchronously writes to this file
    fn write_async<'a>(&'a mut self, buf: &'a [u8], register_root_waker: &'_ Arc<dyn Fn(Waker) + Send + Sync + 'static>) -> Box<dyn Future<Output=io::Result<usize>> + 'a>
    where Self: Sized
    {
        Box::new(VirtualFileAsyncWrite {
            file: self,
            buf,
            register_root_waker: register_root_waker.clone()
        })
    }

    /// Indicates if the file is opened or closed. This function must not block
    /// Defaults to a status of being constantly open
    fn is_open(&self) -> bool {
        true
    }

    /// Returns a special file descriptor when opening this file rather than
    /// generating a new one
    fn get_special_fd(&self) -> Option<u32> {
        None
    }

    /// Used for polling.  Default returns `None` because this method cannot be implemented for most types
    /// Returns the underlying host fd
    fn get_fd(&self) -> Option<FileDescriptor> {
        None
    }
}

struct VirtualFileAsyncRead<'a, T>
{
    file: &'a mut T,
    buf: Option<Vec<u8>>,
    register_root_waker: Arc<dyn Fn(Waker) + Send + Sync + 'static>
}
impl<'a, T> Future
for VirtualFileAsyncRead<'a, T>
where T: VirtualFile
{
    type Output = io::Result<Vec<u8>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.file.poll_read_ready(cx, &self.register_root_waker) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(FsError::WouldBlock)) => { },
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Into::<io::Error>::into(err))),
            Poll::Ready(Ok(_)) => { }
        };
        let mut buf = match self.buf.take() {
            Some(a) => a,
            None => {
                return Poll::Ready(Err(Into::<io::Error>::into(io::ErrorKind::BrokenPipe)));
            }
        };
        unsafe { buf.set_len(buf.capacity()); }
        Poll::Ready(
            self.file.read(&mut buf[..])
                .map(|amt| {
                    unsafe { buf.set_len(amt); }
                    buf
                })
        )
    }
}

struct VirtualFileAsyncWrite<'a, T> {
    file: &'a mut T,
    buf: &'a [u8],
    register_root_waker: Arc<dyn Fn(Waker) + Send + Sync + 'static>
}
impl<'a, T> Future
for VirtualFileAsyncWrite<'a, T>
where T: VirtualFile
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.file.poll_write_ready(cx, &self.register_root_waker) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(FsError::WouldBlock)) => { },
            Poll::Ready(Err(err)) => return Poll::Ready(Err(Into::<io::Error>::into(err))),
            Poll::Ready(Ok(_)) => { }
        };
        let buf = self.buf;
        Poll::Ready(
            self.file.write(buf)
        )
    }
}

// Implementation of `Upcastable` taken from https://users.rust-lang.org/t/why-does-downcasting-not-work-for-subtraits/33286/7 .
/// Trait needed to get downcasting from `VirtualFile` to work.
pub trait Upcastable {
    fn upcast_any_ref(&'_ self) -> &'_ dyn Any;
    fn upcast_any_mut(&'_ mut self) -> &'_ mut dyn Any;
    fn upcast_any_box(self: Box<Self>) -> Box<dyn Any>;
}

pub trait ClonableVirtualFile: VirtualFile + Clone {}

impl<T: Any + fmt::Debug + 'static> Upcastable for T {
    #[inline]
    fn upcast_any_ref(&'_ self) -> &'_ dyn Any {
        self
    }
    #[inline]
    fn upcast_any_mut(&'_ mut self) -> &'_ mut dyn Any {
        self
    }
    #[inline]
    fn upcast_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Determines the mode that stdio handlers will operate in
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StdioMode {
    /// Stdio will be piped to a file descriptor
    Piped,
    /// Stdio will inherit the file handlers of its parent
    Inherit,
    /// Stdio will be dropped
    Null,
    /// Stdio will be sent to the log handler
    Log,
}

/// Error type for external users
#[derive(Error, Copy, Clone, Debug, PartialEq, Eq)]
pub enum FsError {
    /// The fd given as a base was not a directory so the operation was not possible
    #[error("fd not a directory")]
    BaseNotDirectory,
    /// Expected a file but found not a file
    #[error("fd not a file")]
    NotAFile,
    /// The fd given was not usable
    #[error("invalid fd")]
    InvalidFd,
    /// File exists
    #[error("file exists")]
    AlreadyExists,
    /// The filesystem has failed to lock a resource.
    #[error("lock error")]
    Lock,
    /// Something failed when doing IO. These errors can generally not be handled.
    /// It may work if tried again.
    #[error("io error")]
    IOError,
    /// The address was in use
    #[error("address is in use")]
    AddressInUse,
    /// The address could not be found
    #[error("address could not be found")]
    AddressNotAvailable,
    /// A pipe was closed
    #[error("broken pipe (was closed)")]
    BrokenPipe,
    /// The connection was aborted
    #[error("connection aborted")]
    ConnectionAborted,
    /// The connection request was refused
    #[error("connection refused")]
    ConnectionRefused,
    /// The connection was reset
    #[error("connection reset")]
    ConnectionReset,
    /// The operation was interrupted before it could finish
    #[error("operation interrupted")]
    Interrupted,
    /// Invalid internal data, if the argument data is invalid, use `InvalidInput`
    #[error("invalid internal data")]
    InvalidData,
    /// The provided data is invalid
    #[error("invalid input")]
    InvalidInput,
    /// Could not perform the operation because there was not an open connection
    #[error("connection is not open")]
    NotConnected,
    /// The requested file or directory could not be found
    #[error("entry not found")]
    EntryNotFound,
    /// The requested device couldn't be accessed
    #[error("can't access device")]
    NoDevice,
    /// Caller was not allowed to perform this operation
    #[error("permission denied")]
    PermissionDenied,
    /// The operation did not complete within the given amount of time
    #[error("time out")]
    TimedOut,
    /// Found EOF when EOF was not expected
    #[error("unexpected eof")]
    UnexpectedEof,
    /// Operation would block, this error lets the caller know that they can try again
    #[error("blocking operation. try again")]
    WouldBlock,
    /// A call to write returned 0
    #[error("write returned 0")]
    WriteZero,
    /// Directory not Empty
    #[error("directory not empty")]
    DirectoryNotEmpty,
    /// Some other unhandled error. If you see this, it's probably a bug.
    #[error("unknown error found")]
    UnknownError,
}

impl From<io::Error> for FsError {
    fn from(io_error: io::Error) -> Self {
        match io_error.kind() {
            io::ErrorKind::AddrInUse => FsError::AddressInUse,
            io::ErrorKind::AddrNotAvailable => FsError::AddressNotAvailable,
            io::ErrorKind::AlreadyExists => FsError::AlreadyExists,
            io::ErrorKind::BrokenPipe => FsError::BrokenPipe,
            io::ErrorKind::ConnectionAborted => FsError::ConnectionAborted,
            io::ErrorKind::ConnectionRefused => FsError::ConnectionRefused,
            io::ErrorKind::ConnectionReset => FsError::ConnectionReset,
            io::ErrorKind::Interrupted => FsError::Interrupted,
            io::ErrorKind::InvalidData => FsError::InvalidData,
            io::ErrorKind::InvalidInput => FsError::InvalidInput,
            io::ErrorKind::NotConnected => FsError::NotConnected,
            io::ErrorKind::NotFound => FsError::EntryNotFound,
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied,
            io::ErrorKind::TimedOut => FsError::TimedOut,
            io::ErrorKind::UnexpectedEof => FsError::UnexpectedEof,
            io::ErrorKind::WouldBlock => FsError::WouldBlock,
            io::ErrorKind::WriteZero => FsError::WriteZero,
            io::ErrorKind::Other => FsError::IOError,
            // if the following triggers, a new error type was added to this non-exhaustive enum
            _ => FsError::UnknownError,
        }
    }
}

impl Into<io::Error> for FsError {
    fn into(self) -> io::Error {
        let kind = match self {
            FsError::AddressInUse => io::ErrorKind::AddrInUse,
            FsError::AddressNotAvailable => io::ErrorKind::AddrNotAvailable,
            FsError::AlreadyExists => io::ErrorKind::AlreadyExists,
            FsError::BrokenPipe => io::ErrorKind::BrokenPipe,
            FsError::ConnectionAborted => io::ErrorKind::ConnectionAborted,
            FsError::ConnectionRefused => io::ErrorKind::ConnectionRefused,
            FsError::ConnectionReset => io::ErrorKind::ConnectionReset,
            FsError::Interrupted => io::ErrorKind::Interrupted,
            FsError::InvalidData => io::ErrorKind::InvalidData,
            FsError::InvalidInput => io::ErrorKind::InvalidInput,
            FsError::NotConnected => io::ErrorKind::NotConnected,
            FsError::EntryNotFound => io::ErrorKind::NotFound,
            FsError::PermissionDenied => io::ErrorKind::PermissionDenied,
            FsError::TimedOut => io::ErrorKind::TimedOut,
            FsError::UnexpectedEof => io::ErrorKind::UnexpectedEof,
            FsError::WouldBlock => io::ErrorKind::WouldBlock,
            FsError::WriteZero => io::ErrorKind::WriteZero,
            FsError::IOError => io::ErrorKind::Other,
            FsError::BaseNotDirectory => io::ErrorKind::Other,
            FsError::NotAFile => io::ErrorKind::Other,
            FsError::InvalidFd => io::ErrorKind::Other,
            FsError::Lock => io::ErrorKind::Other,
            FsError::NoDevice => io::ErrorKind::Other,
            FsError::DirectoryNotEmpty => io::ErrorKind::Other,
            FsError::UnknownError => io::ErrorKind::Other,
        };
        kind.into()
    }
}

#[derive(Debug)]
pub struct ReadDir {
    // TODO: to do this properly we need some kind of callback to the core FS abstraction
    data: Vec<DirEntry>,
    index: usize,
}

impl ReadDir {
    pub fn new(data: Vec<DirEntry>) -> Self {
        Self { data, index: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    // weird hack, to fix this we probably need an internal trait object or callbacks or something
    pub metadata: Result<Metadata>,
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn metadata(&self) -> Result<Metadata> {
        self.metadata.clone()
    }

    pub fn file_type(&self) -> Result<FileType> {
        let metadata = self.metadata.clone()?;
        Ok(metadata.file_type())
    }

    pub fn file_name(&self) -> OsString {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_owned()
    }
}

#[allow(clippy::len_without_is_empty)] // Clippy thinks it's an iterator.
#[derive(Clone, Debug, Default)]
// TODO: review this, proper solution would probably use a trait object internally
pub struct Metadata {
    pub ft: FileType,
    pub accessed: u64,
    pub created: u64,
    pub modified: u64,
    pub len: u64,
}

impl Metadata {
    pub fn is_file(&self) -> bool {
        self.ft.is_file()
    }

    pub fn is_dir(&self) -> bool {
        self.ft.is_dir()
    }

    pub fn accessed(&self) -> u64 {
        self.accessed
    }

    pub fn created(&self) -> u64 {
        self.created
    }

    pub fn modified(&self) -> u64 {
        self.modified
    }

    pub fn file_type(&self) -> FileType {
        self.ft.clone()
    }

    pub fn len(&self) -> u64 {
        self.len
    }
}

#[derive(Clone, Debug, Default)]
// TODO: review this, proper solution would probably use a trait object internally
pub struct FileType {
    pub dir: bool,
    pub file: bool,
    pub symlink: bool,
    // TODO: the following 3 only exist on unix in the standard FS API.
    // We should mirror that API and extend with that trait too.
    pub char_device: bool,
    pub block_device: bool,
    pub socket: bool,
    pub fifo: bool,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.dir
    }
    pub fn is_file(&self) -> bool {
        self.file
    }
    pub fn is_symlink(&self) -> bool {
        self.symlink
    }
    pub fn is_char_device(&self) -> bool {
        self.char_device
    }
    pub fn is_block_device(&self) -> bool {
        self.block_device
    }
    pub fn is_socket(&self) -> bool {
        self.socket
    }
    pub fn is_fifo(&self) -> bool {
        self.fifo
    }
}

impl Iterator for ReadDir {
    type Item = Result<DirEntry>;

    fn next(&mut self) -> Option<Result<DirEntry>> {
        if let Some(v) = self.data.get(self.index).cloned() {
            self.index += 1;
            return Some(Ok(v));
        }
        None
    }
}
