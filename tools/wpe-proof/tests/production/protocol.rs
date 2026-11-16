use crate::page::platform::{
    presentation::{Configuration, Received, Surface},
    transport::{Outbox, Packet, Reader},
};
use gpui::{native_view::Viewport, px, size};
use serde_json::{Value, json};
use std::{
    io::{Read as _, Write as _},
    os::{
        fd::{FromRawFd as _, OwnedFd},
        unix::net::UnixStream,
    },
    time::Duration,
};

#[test]
fn truncated_and_oversized_packets_fail_without_allocating_payloads() {
    for header in [
        vec![b'S', 1],
        [
            vec![b'S'],
            1u32.to_le_bytes().to_vec(),
            u32::MAX.to_le_bytes().to_vec(),
        ]
        .concat(),
    ] {
        let (mut sender, receiver) = UnixStream::pair().unwrap();
        sender.write_all(&header).unwrap();
        drop(sender);
        assert!(Reader::new(receiver).next().is_err());
    }
}

#[test]
fn stale_and_discarded_frames_return_each_buffer_once() {
    let (host, mut helper) = UnixStream::pair().unwrap();
    helper
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let outbox = Outbox::new(host).unwrap();
    let configuration = Configuration {
        viewport: Viewport::try_from(size(px(2.0), px(2.0))).unwrap(),
        scale: 1.0,
        generation: 2,
    };
    let mut surface = Surface::default();
    for generation in [1, 2] {
        // SAFETY: the name is NUL terminated; ownership transfers immediately.
        let raw = unsafe { libc::memfd_create(c"bezel-frame-test".as_ptr(), libc::MFD_CLOEXEC) };
        assert!(raw >= 0);
        let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
        surface.receive(Some(configuration), &outbox, Packet {
            kind: b'B', page: 1,
            bytes: serde_json::to_vec(&json!({"buffer":generation,"width":2,"height":2,"stride":8,"offset":0,"fourcc":u32::from_le_bytes(*b"AR24"),"kind":"shm"})).unwrap(),
            descriptor: Some(descriptor),
        }).unwrap();
        let received = surface
            .receive(
                Some(configuration),
                &outbox,
                Packet {
                    kind: b'F',
                    page: 1,
                    bytes: serde_json::to_vec(
                        &json!({"buffer":generation,"generation":generation}),
                    )
                    .unwrap(),
                    descriptor: None,
                },
            )
            .unwrap();
        assert!(matches!(
            (generation, received),
            (1, Received::Unchanged) | (2, Received::Changed)
        ));
    }
    surface.clear();
    for (buffer, command) in [
        (1, "presented"),
        (1, "released"),
        (2, "presented"),
        (2, "released"),
    ] {
        let mut length = [0; 4];
        helper.read_exact(&mut length).unwrap();
        let mut payload = vec![0; u32::from_le_bytes(length) as usize];
        helper.read_exact(&mut payload).unwrap();
        let value: Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(value["buffer"], buffer);
        assert_eq!(value["cmd"], command);
    }
    outbox.close();
    let mut byte = [0];
    assert_eq!(helper.read(&mut byte).unwrap(), 0);
}
