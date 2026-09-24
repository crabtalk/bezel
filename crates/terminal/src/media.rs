//! The kitty graphics mediums that name a payload rather than carry it:
//! a file (`t=f`), a temporary file (`t=t`) and a POSIX shared memory object
//! (`t=s`).
//!
//! The name comes from whatever is on the other end of the pty, which can be
//! a remote or sandboxed program. Every failure is therefore the one error,
//! [`Unreadable`], so the reply cannot tell a program anything about a file it
//! could not read itself.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// A payload that could not be read, for any reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

/// Where a medium's bytes start, and how many of them to take. A `len` of
/// zero reads to the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub offset: u64,
    pub len: u64,
}

/// The substring a temporary file's path must carry to be deleted after it
/// is read.
const TEMP_MARKER: &str = "tty-graphics-protocol";

/// Read the payload `name` stands for under `medium`, at most `max` bytes.
pub fn read(medium: char, name: &[u8], span: Span, max: usize) -> Result<Vec<u8>, Unreadable> {
    let name = std::str::from_utf8(name).map_err(|_| Unreadable)?;
    match medium {
        'f' => read_file(&resolve(name)?, span, max),
        't' => {
            let path = resolve(name)?;
            let bytes = read_file(&path, span, max);
            if is_temporary(&path) {
                let _ = std::fs::remove_file(&path);
            }
            bytes
        }
        's' => read_shared(name, span, max),
        _ => Err(Unreadable),
    }
}

/// The path a name resolves to, symlinks followed, refused when it lands in
/// a part of the filesystem whose files are not files. Checked before
/// anything opens it: opening one of those can itself do something.
fn resolve(name: &str) -> Result<PathBuf, Unreadable> {
    let path = Path::new(name);
    if !path.is_absolute() {
        return Err(Unreadable);
    }
    let path = path.canonicalize().map_err(|_| Unreadable)?;
    let sensitive = ["/proc", "/sys", "/dev"]
        .iter()
        .any(|root| path.starts_with(root));
    if sensitive && !path.starts_with("/dev/shm") {
        return Err(Unreadable);
    }
    if !path.metadata().map_err(|_| Unreadable)?.is_file() {
        return Err(Unreadable);
    }
    Ok(path)
}

fn read_file(path: &Path, span: Span, max: usize) -> Result<Vec<u8>, Unreadable> {
    let mut file = File::open(path).map_err(|_| Unreadable)?;
    file.seek(SeekFrom::Start(span.offset))
        .map_err(|_| Unreadable)?;
    let limit = match span.len {
        0 => max as u64 + 1,
        len => len.min(max as u64 + 1),
    };
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| Unreadable)?;
    if bytes.len() > max || (span.len != 0 && (bytes.len() as u64) < span.len) {
        return Err(Unreadable);
    }
    Ok(bytes)
}

/// Whether a temporary file is one the terminal may delete: under a known
/// temporary directory, with [`TEMP_MARKER`] in its path.
fn is_temporary(path: &Path) -> bool {
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/dev/shm"),
        std::env::temp_dir(),
    ];
    if let Some(dir) = std::env::var_os("TMPDIR") {
        roots.push(PathBuf::from(dir));
    }
    let under = roots.iter().any(|root| {
        let root = root.canonicalize().unwrap_or_else(|_| root.clone());
        path.starts_with(root)
    });
    under && path.to_string_lossy().contains(TEMP_MARKER)
}

/// A POSIX shared memory object, unlinked once read whether or not the read
/// succeeded.
#[cfg(unix)]
fn read_shared(name: &str, span: Span, max: usize) -> Result<Vec<u8>, Unreadable> {
    use std::ffi::CString;

    if !name.starts_with('/') || name[1..].contains('/') {
        return Err(Unreadable);
    }
    let c_name = CString::new(name).map_err(|_| Unreadable)?;
    // SAFETY: `c_name` is a valid C string for the duration of each call.
    let fd = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDONLY, 0) };
    if fd < 0 {
        return Err(Unreadable);
    }
    let bytes = map_and_copy(fd, span, max);
    // SAFETY: `fd` is the descriptor `shm_open` returned and nothing else
    // closes it; `c_name` is valid as above.
    unsafe {
        libc::close(fd);
        libc::shm_unlink(c_name.as_ptr());
    }
    bytes
}

/// The object's bytes, through `mmap`: macOS does not `read` a shared
/// memory descriptor.
#[cfg(unix)]
fn map_and_copy(fd: libc::c_int, span: Span, max: usize) -> Result<Vec<u8>, Unreadable> {
    // SAFETY: `stat` is plain data, and `fstat` fills it for a valid `fd`.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        return Err(Unreadable);
    }
    let size = u64::try_from(stat.st_size).map_err(|_| Unreadable)?;
    if span.offset >= size {
        return Err(Unreadable);
    }
    let available = size - span.offset;
    let len = match span.len {
        0 => available,
        len if len <= available => len,
        _ => return Err(Unreadable),
    };
    if len > max as u64 {
        return Err(Unreadable);
    }
    let size = usize::try_from(size).map_err(|_| Unreadable)?;
    // SAFETY: a read-only shared mapping of the whole object, unmapped below
    // before anything else can see it.
    let map = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    if map == libc::MAP_FAILED {
        return Err(Unreadable);
    }
    // SAFETY: `offset + len <= size`, the length of the live mapping.
    let bytes = unsafe {
        std::slice::from_raw_parts((map as *const u8).add(span.offset as usize), len as usize)
    }
    .to_vec();
    // SAFETY: `map` and `size` are the pair `mmap` returned.
    unsafe { libc::munmap(map, size) };
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_shared(_: &str, _: Span, _: usize) -> Result<Vec<u8>, Unreadable> {
    Err(Unreadable)
}
