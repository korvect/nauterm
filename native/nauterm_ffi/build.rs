use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_ZIG_VERSION: &str = "0.16.0";
const GHOSTTY_REPOSITORY: &str = "https://github.com/ghostty-org/ghostty.git";
const GHOSTTY_COMMIT: &str = "f64f4aca2c29b554d111b36c3d946a9bddd159ff";
const GHOSTTY_VERSION: &str = "1.3.2-dev";

fn main() {
    println!("cargo:rerun-if-env-changed=NAUTERM_ZIG");
    println!("cargo:rerun-if-env-changed=NAUTERM_GHOSTTY_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=NAUTERM_GHOSTTY_VT_LIB_DIR");

    if env::var_os("CARGO_FEATURE_TERMINAL_GHOSTTY").is_none() {
        return;
    }

    let cargo_out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    if let Some(lib_dir) = env::var_os("NAUTERM_GHOSTTY_VT_LIB_DIR") {
        link_ghostty(Path::new(&lib_dir), &cargo_out_dir);
        return;
    }

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
            "--verbose",
            "-Demit-lib-vt=true",
            "-Demit-xcframework=false",
            "-Doptimize=ReleaseFast",
            &format!("-Dversion-string={GHOSTTY_VERSION}"),
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
        panic!(
            "Zig failed to build libghostty-vt for {zig_target} ({})",
            output.status
        );
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
    link_ghostty(&out_dir.join("lib"), &cargo_out_dir);
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
        remove_ghostty_git_metadata(&source_dir);
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
    remove_ghostty_git_metadata(&source_dir);

    source_dir
}

fn remove_ghostty_git_metadata(source_dir: &Path) {
    let git_dir = source_dir.join(".git");
    if !git_dir.exists() {
        return;
    }
    std::fs::remove_dir_all(&git_dir).unwrap_or_else(|error| {
        panic!(
            "failed to remove Ghostty Git metadata at {}: {error}",
            git_dir.display()
        )
    });
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
        // Keep the target explicit instead of using native-native-msvc. This
        // avoids host CPU detection in Zig on the Windows ARM runner while
        // still using the runner's native ARM64 MSVC SDK.
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        other => panic!("unsupported libghostty-vt target: {other}"),
    }
}

fn link_ghostty(lib_dir: &Path, cargo_out_dir: &Path) {
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        link_ghostty_windows(lib_dir, cargo_out_dir);
        return;
    }

    let archive = lib_dir.join("libghostty-vt.a");
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

fn link_ghostty_windows(lib_dir: &Path, cargo_out_dir: &Path) {
    let dll = [
        lib_dir.join("ghostty-vt.dll"),
        lib_dir
            .parent()
            .unwrap_or(lib_dir)
            .join("bin")
            .join("ghostty-vt.dll"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
    .unwrap_or_else(|| {
        panic!(
            "libghostty-vt runtime DLL not found beside {} or in its sibling bin directory",
            lib_dir.display()
        )
    });

    // Keep Zig's compiler runtime inside ghostty-vt.dll. Linking the static
    // archive into Rust on Windows puts Zig compiler_rt and Rust
    // compiler_builtins in the same image, where MSVC rejects conflicting
    // weak extern defaults (LNK1227 on ARM64). The Rust extern block uses
    // raw-dylib so rustc generates a clean import library directly from the
    // C declarations instead of consuming Zig's CRT-affecting import library.
    stage_windows_runtime_dll(&dll, cargo_out_dir);
}

fn stage_windows_runtime_dll(dll: &Path, cargo_out_dir: &Path) {
    // OUT_DIR is <cargo-target>/<profile>/build/<package-hash>/out. Cargo test
    // executes binaries from <profile>/deps, while Flutter and the packaging
    // scripts load nauterm_ffi from <profile>. Put the dependency in both
    // locations so every build mode uses Windows' normal adjacent-DLL lookup.
    let profile_dir = cargo_out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or_else(|| {
            panic!(
                "unable to resolve Cargo profile directory from OUT_DIR={}",
                cargo_out_dir.display()
            )
        });
    for destination_dir in [profile_dir.to_path_buf(), profile_dir.join("deps")] {
        std::fs::create_dir_all(&destination_dir).unwrap_or_else(|error| {
            panic!(
                "failed to create Ghostty runtime directory {}: {error}",
                destination_dir.display()
            )
        });
        let destination = destination_dir.join("ghostty-vt.dll");
        std::fs::copy(dll, &destination).unwrap_or_else(|error| {
            panic!(
                "failed to stage {} at {}: {error}",
                dll.display(),
                destination.display()
            )
        });
    }
}
