use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/page/linux/helper.c");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    let packages = [
        "wpe-webkit-2.0 >= 2.54",
        "wpe-platform-headless-2.0",
        "json-glib-1.0",
        "xkbcommon",
    ];
    let flags = Command::new(env::var_os("PKG_CONFIG").unwrap_or_else(|| "pkg-config".into()))
        .args(["--cflags", "--libs"])
        .args(packages)
        .output()
        .expect("pkg-config is required to build the WPE helper");
    assert!(
        flags.status.success(),
        "WPE WebKit 2.54+ and WPEPlatform development files are required: {}",
        String::from_utf8_lossy(&flags.stderr)
    );
    let compiler = cc::Build::new().get_compiler();
    let mut command = compiler.to_command();
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bezel-wpe");
    command
        .args([
            "-std=gnu11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Wno-unused-parameter",
            "src/page/linux/helper.c",
            "-o",
        ])
        .arg(output)
        .args(
            String::from_utf8(flags.stdout)
                .expect("UTF-8 pkg-config flags")
                .split_whitespace(),
        )
        .arg("-lm");
    assert!(
        command.status().expect("build WPE helper").success(),
        "WPE helper compilation failed"
    );
}
