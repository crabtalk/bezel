use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, Write as _},
    os::{
        fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
/// The helper drops larger packets.
const MAX_PACKET: usize = 8 * 1024 * 1024;
/// Commands to the helper, written by a dedicated thread: GPU completion
/// callbacks and the UI thread must never block on a busy helper.
#[derive(Clone)]
pub(super) struct Outbox {
    sender: mpsc::SyncSender<Vec<u8>>,
    socket: Arc<UnixStream>,
    queued: Arc<AtomicUsize>,
}

impl Outbox {
    pub(super) fn send(&self, id: u32, mut command: Value) -> Result<(), String> {
        command["id"] = id.into();
        let data = serde_json::to_vec(&command).map_err(|e| e.to_string())?;
        if data.len() > MAX_PACKET {
            return Err("Browser command is too large".into());
        }
        let mut framed = Vec::with_capacity(data.len() + 4);
        framed.extend_from_slice(&(data.len() as u32).to_le_bytes());
        framed.extend_from_slice(&data);
        let length = framed.len();
        if self
            .queued
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                n.checked_add(length).filter(|n| *n <= 16 * 1024 * 1024)
            })
            .is_err()
        {
            self.close();
            return Err("Browser command queue is full".into());
        }
        if self.sender.try_send(framed).is_err() {
            self.queued.fetch_sub(length, Ordering::Relaxed);
            self.close();
            return Err("Browser helper is not accepting commands".into());
        }
        Ok(())
    }
    /// Closes the helper's input even while completions still hold clones.
    pub(super) fn close(&self) {
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}

pub(super) fn writer(socket: UnixStream) -> io::Result<Outbox> {
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(64);
    let connection = Arc::new(socket.try_clone()?);
    let queued = Arc::new(AtomicUsize::new(0));
    let remaining = queued.clone();
    std::thread::Builder::new()
        .name("browser-commands".into())
        .spawn(move || {
            let mut socket = socket;
            for message in rx {
                let result = socket.write_all(&message);
                remaining.fetch_sub(message.len(), Ordering::Relaxed);
                if result.is_err() {
                    break;
                }
            }
            let _ = socket.shutdown(std::net::Shutdown::Write);
        })?;
    Ok(Outbox {
        sender: tx,
        socket: connection,
        queued,
    })
}

/// Reads packets and the descriptors attached to them. A descriptor arrives
/// with the first byte of its packet, so it is queued no later than the packet
/// that claims it completes.
pub(super) struct Reader {
    socket: UnixStream,
    data: Vec<u8>,
    fds: VecDeque<OwnedFd>,
}

impl Reader {
    pub(super) fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            data: Vec::new(),
            fds: VecDeque::new(),
        }
    }

    pub(super) fn next(&mut self) -> io::Result<(u8, u32, Vec<u8>)> {
        loop {
            if self.data.len() >= 9 {
                let length = u32::from_le_bytes(self.data[5..9].try_into().unwrap()) as usize;
                if length > MAX_PACKET {
                    return Err(io::Error::other("Browser packet is too large"));
                }
                if self.data.len() >= 9 + length {
                    let kind = self.data[0];
                    let id = u32::from_le_bytes(self.data[1..5].try_into().unwrap());
                    let payload = self.data[9..9 + length].to_vec();
                    self.data.drain(..9 + length);
                    return Ok((kind, id, payload));
                }
            }
            self.receive()?;
        }
    }

    pub(super) fn take_fd(&mut self) -> io::Result<OwnedFd> {
        self.fds
            .pop_front()
            .ok_or_else(|| io::Error::other("Browser packet is missing its descriptor"))
    }

    fn receive(&mut self) -> io::Result<()> {
        let mut chunk = vec![0u8; 64 * 1024];
        // Room for 16 descriptors, aligned for cmsghdr.
        let mut control = [0u64; 16];
        let mut iov = libc::iovec {
            iov_base: chunk.as_mut_ptr().cast(),
            iov_len: chunk.len(),
        };
        // SAFETY: an all-zero msghdr is valid; the pointers outlive the call.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut iov;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = std::mem::size_of_val(&control) as _;
        let n = loop {
            // SAFETY: `message` describes buffers owned by this frame.
            let n = unsafe {
                libc::recvmsg(
                    self.socket.as_raw_fd(),
                    &mut message,
                    libc::MSG_CMSG_CLOEXEC,
                )
            };
            if n < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break n;
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the kernel filled `control`; the CMSG macros walk it in bounds.
        unsafe {
            let mut header = libc::CMSG_FIRSTHDR(&message);
            while !header.is_null() {
                if (*header).cmsg_level == libc::SOL_SOCKET
                    && (*header).cmsg_type == libc::SCM_RIGHTS
                {
                    let bytes = (*header).cmsg_len as usize - libc::CMSG_LEN(0) as usize;
                    let data = libc::CMSG_DATA(header) as *const RawFd;
                    for i in 0..bytes / std::mem::size_of::<RawFd>() {
                        self.fds
                            .push_back(OwnedFd::from_raw_fd(data.add(i).read_unaligned()));
                    }
                }
                header = libc::CMSG_NXTHDR(&message, header);
            }
        }
        if message.msg_flags & libc::MSG_CTRUNC != 0 {
            return Err(io::Error::other("Browser helper sent too many descriptors"));
        }
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if self.fds.len() > 64 {
            return Err(io::Error::other("Browser sent too many descriptors"));
        }
        self.data.extend_from_slice(&chunk[..n as usize]);
        Ok(())
    }
}
