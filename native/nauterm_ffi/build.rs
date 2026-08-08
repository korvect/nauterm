use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_ZIG_VERSION: &str = "0.16.0";
const GHOSTTY_REPOSITORY: &str = "https://github.com/ghostty-org/ghostty.git";
const GHOSTTY_COMMIT: &str = "2602886144c7e95099c9e2ba07f181c69e7276f3";

fn main() {
    println!("cargo:rerun-if-env-changed=NAUTERM_ZIG");
    println!("cargo:rerun-if-env-changed=NAUTERM_GHOSTTY_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=NAUTERM_GHOSTTY_VT_LIB_DIR");

    if env::var_os("CARGO_FEATURE_TERMINAL_GHOSTTY").is_none() {
        return;
    }

    if let Some(lib_dir) = env::var_os("NAUTERM_GHOSTTY_VT_LIB_DIR") {
        link_ghostty(Path::new(&lib_dir));
        return;
    }

    let cargo_out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let ghostty_root = match env::var_os("NAUTERM_GHOSTTY_SOURCE_DIR") {
        Some(source_dir) => {
            let source_dir = PathBuf::from(source_dir);
            if !source_dir.join("build.zig").is_file() {
                panic!(
                    "NAUTERM_GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
                    source_dir.display()
                );
            }
            source_dir
        }
        None => fetch_ghostty(&cargo_out_dir),
    };
    let build_zig = ghostty_root.join("build.zig");

    let zig = env::var_os("NAUTERM_ZIG").unwrap_or_else(|| "zig".into());
    verify_zig_version(&zig);

    let target = env::var("TARGET").expect("Cargo TARGET is required");
    let zig_target = zig_target(&target);
    let out_dir = cargo_out_dir.join("ghostty-vt");
    let output = Command::new(&zig)
        .current_dir(&ghostty_root)
        .args([
            "build",
            "-Demit-lib-vt=true",
            "-Demit-xcframework=false",
            "-Doptimize=ReleaseFast",
            &format!("-Dtarget={zig_target}"),
            "--prefix",
        ])
        .arg(&out_dir)
        .output()
        .expect("failed to execute Zig while building libghostty-vt");
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Zig failed to build libghostty-vt for {zig_target}.");
        if !stdout.trim().is_empty() {
            eprintln!("Zig stdout:\n{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprintln!("Zig stderr:\n{stderr}");
        }
        panic!("Zig failed to build libghostty-vt for {zig_target}");
    }

    println!("cargo:rerun-if-changed={}", build_zig.display());
    println!(
        "cargo:rerun-if-changed={}",
        ghostty_root.join("src/terminal").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        ghostty_root.join("include/ghostty/vt.h").display()
    );
    link_ghostty(&out_dir.join("lib"));
}

fn verify_zig_version(zig: &std::ffi::OsStr) {
    let output = Command::new(zig)
        .arg("version")
        .output()
        .expect("Zig is required to build libghostty-vt");
    let version = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || version.trim() != REQUIRED_ZIG_VERSION {
        panic!(
            "libghostty-vt requires Zig {REQUIRED_ZIG_VERSION}; found `{}`",
            version.trim()
        );
    }
}

fn fetch_ghostty(out_dir: &Path) -> PathBuf {
    let source_dir = out_dir.join("ghostty-src");
    let commit_stamp = source_dir.join(".nauterm-ghostty-commit");

    let cached_commit_matches = std::fs::read_to_string(&commit_stamp)
        .map(|existing_commit| existing_commit.trim() == GHOSTTY_COMMIT)
        .unwrap_or(false);
    if source_dir.join("build.zig").is_file() && cached_commit_matches {
        return source_dir;
    }

    if source_dir.exists() {
        std::fs::remove_dir_all(&source_dir).unwrap_or_else(|error| {
            panic!(
                "failed to remove stale Ghostty source at {}: {error}",
                source_dir.display()
            )
        });
    }

    eprintln!("Fetching Ghostty commit {GHOSTTY_COMMIT}...");
    run(
        Command::new("git")
            .args(["clone", "--filter=blob:none", "--no-checkout"])
            .arg(GHOSTTY_REPOSITORY)
            .arg(&source_dir),
        "clone Ghostty repository",
    );
    run(
        Command::new("git")
            .args(["checkout", GHOSTTY_COMMIT])
            .current_dir(&source_dir),
        "checkout pinned Ghostty commit",
    );
    std::fs::write(&commit_stamp, GHOSTTY_COMMIT).unwrap_or_else(|error| {
        panic!(
            "failed to record Ghostty commit at {}: {error}",
            commit_stamp.display()
        )
    });

    source_dir
}

fn run(command: &mut Command, description: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to {description}: {error}"));
    if !status.success() {
        panic!("failed to {description}: {status}");
    }
}

fn zig_target(target: &str) -> &'static str {
    match target {
        "aarch64-apple-darwin" => "aarch64-macos",
        "x86_64-apple-darwin" => "x86_64-macos",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        other => panic!("unsupported libghostty-vt target: {other}"),
    }
}

fn link_ghostty(lib_dir: &Path) {
    let archive = if env::var("TARGET").unwrap_or_default().contains("windows") {
        lib_dir.join("ghostty-vt-static.lib")
    } else {
        lib_dir.join("libghostty-vt.a")
    };
    if !archive.is_file() {
        panic!(
            "libghostty-vt static archive not found: {}",
            archive.display()
        );
    }
    // Pass the archive by exact path. Using `-lghostty-vt` is ambiguous on
    // Apple platforms because the Zig install also contains the shared
    // library, which can leave libnauterm_ffi with an unintended @rpath
    // dependency even when a static link kind is requested.
    println!("cargo:rustc-link-arg={}", archive.display());
}
