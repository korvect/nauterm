use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/console_hook.c");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // Use Cargo's target compiler, including the ARM64 developer environment
    // in CI. Both artifacts have the same architecture as CMD and the client.
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let compiler = cc::Build::new().static_crt(true).get_compiler();
    assert!(compiler.is_like_msvc(), "CMD integration requires MSVC");
    for (name, launcher) in [("nauterm-cmd-hook.dll", false), ("nauterm-cmd.exe", true)] {
        let mut command = compiler.to_command();
        command.args(["/nologo", "/O2", "/W4", "/utf-8"]);
        if launcher {
            command.arg("/DNAUTERM_LAUNCHER");
        } else {
            command.arg("/LD");
        }
        command.arg("src/console_hook.c");
        command.arg(format!("/Fo{}", out.join(format!("{name}.obj")).display()));
        command.arg(format!("/Fe{}", out.join(name).display()));
        command.arg("/link").arg("kernel32.lib");
        if launcher {
            command.arg("/SUBSYSTEM:CONSOLE");
        }
        let status = command.status().expect("run CMD integration compiler");
        assert!(status.success(), "compile {name}: {status}");
    }
}
