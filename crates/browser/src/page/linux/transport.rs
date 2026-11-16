use serde_json::Value;
use std::{
    io::{self, Read as _, Write as _},
    net::Shutdown,
    os::{
        fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{Arc, Mutex, mpsc},
    thread,
};

const MAX_PACKET: usize = 8 * 1024 * 1024;
const COMMAND_CAPACITY: usize = 64;
const COMMAND_BYTES: usize = 16 * 1024 * 1024;

struct Queued {
    bytes: Vec<u8>,
    budget: Arc<Mutex<usize>>,
}
impl Drop for Queued {
    fn drop(&mut self) {
        *self.budget.lock().unwrap() -= self.bytes.len();
    }
}

#[derive(Clone)]
pub(super) struct Outbox {
    commands: mpsc::SyncSender<Queued>,
    connection: Arc<UnixStream>,
    budget: Arc<Mutex<usize>>,
}

impl Outbox {
    pub(super) fn send(&self, page: u32, mut command: Value) -> Result<(), String> {
        command["id"] = page.into();
        let payload = serde_json::to_vec(&command).map_err(|error| error.to_string())?;
        if payload.len() > MAX_PACKET {
            return Err("Browser command exceeds the packet limit".into());
        }
        let mut packet = Vec::with_capacity(payload.len() + 4);
        packet.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        packet.extend(payload);
        let mut used = self.budget.lock().unwrap();
        if *used + packet.len() > COMMAND_BYTES {
            self.close();
            return Err("Browser command byte budget exceeded".into());
        }
        *used += packet.len();
        drop(used);
        let queued = Queued {
            bytes: packet,
            budget: self.budget.clone(),
        };
        self.commands.try_send(queued).map_err(|error| {
            self.close();
            format!("Browser command delivery failed: {error}")
        })
    }

    pub(super) fn close(&self) {
        let _ = self.connection.shutdown(Shutdown::Both);
    }
}

impl Outbox {
    pub(super) fn new(mut connection: UnixStream) -> io::Result<Self> {
        let (commands, queued) = mpsc::sync_channel::<Queued>(COMMAND_CAPACITY);
        let shutdown = Arc::new(connection.try_clone()?);
        thread::Builder::new()
            .name("wpe-commands".into())
            .spawn(move || {
                for command in queued {
                    if connection.write_all(&command.bytes).is_err() {
                        break;
                    }
                }
                let _ = connection.shutdown(Shutdown::Both);
            })?;
        Ok(Outbox {
            commands,
            connection: shutdown,
            budget: Arc::new(Mutex::new(0)),
        })
    }
}
pub(super) struct Packet {
    pub(super) kind: u8,
    pub(super) page: u32,
    pub(super) bytes: Vec<u8>,
    pub(super) descriptor: Option<OwnedFd>,
}
pub(super) struct Reader {
    connection: UnixStream,
}

impl Reader {
    pub(super) fn new(connection: UnixStream) -> Self {
        Self { connection }
    }

    pub(super) fn next(&mut self) -> io::Result<Packet> {
        let mut kind = [0];
        let mut rights = [0u64; 4];
        let mut vector = libc::iovec {
            iov_base: kind.as_mut_ptr().cast(),
            iov_len: 1,
        };
        // SAFETY: all pointers refer to writable buffers alive through recvmsg.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = rights.as_mut_ptr().cast();
        loop {
            message.msg_controllen = std::mem::size_of_val(&rights);
            message.msg_flags = 0;
            // SCM_RIGHTS belongs to the first byte, which is read without read-ahead.
            let count = unsafe {
                libc::recvmsg(
                    self.connection.as_raw_fd(),
                    &mut message,
                    libc::MSG_CMSG_CLOEXEC,
                )
            };
            if count == 1 {
                break;
            }
            if count == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        let mut descriptors = Vec::new();
        // SAFETY: CMSG iteration stays within the kernel-populated control buffer.
        unsafe {
            let mut item = libc::CMSG_FIRSTHDR(&message);
            while !item.is_null() {
                if (*item).cmsg_level == libc::SOL_SOCKET && (*item).cmsg_type == libc::SCM_RIGHTS {
                    let length = (*item).cmsg_len.saturating_sub(libc::CMSG_LEN(0) as usize);
                    let data = libc::CMSG_DATA(item).cast::<RawFd>();
                    for index in 0..length / std::mem::size_of::<RawFd>() {
                        descriptors.push(OwnedFd::from_raw_fd(data.add(index).read_unaligned()));
                    }
                }
                item = libc::CMSG_NXTHDR(&message, item);
            }
        }
        if message.msg_flags & libc::MSG_CTRUNC != 0 || descriptors.len() > 1 {
            return Err(io::Error::other("Invalid browser descriptor count"));
        }
        let descriptor = descriptors.pop();
        let mut header = [0; 8];
        self.connection.read_exact(&mut header)?;
        let page = u32::from_le_bytes(header[..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
        if length > MAX_PACKET {
            return Err(io::Error::other(
                "Browser response exceeds the packet limit",
            ));
        }
        if descriptor.is_some() != matches!(kind[0], b'B' | b'P') {
            return Err(io::Error::other(
                "Browser response has an unexpected descriptor",
            ));
        }
        let mut payload = vec![0; length];
        self.connection.read_exact(&mut payload)?;
        Ok(Packet {
            kind: kind[0],
            page,
            bytes: payload,
            descriptor,
        })
    }
}
