#![cfg(target_os = "linux")]
mod page {
    mod platform {
        mod transport {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../crates/browser/src/page/linux/transport.rs"
            ));
        }
        mod presentation {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../crates/browser/src/page/linux/presentation.rs"
            ));
        }
        mod protocol {
            include!("production/protocol.rs");
        }
        mod cases {
            include!("production/cases.rs");
        }
    }
}
