//! Embed the icon GPUI loads from resource ID 1 and the package version.
use std::{env, fs, path::Path, process::Command};

pub fn compile(root: &Path, build: u16, application_name: &str) {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let metadata_path = root.join("windows/application.json");
    println!("cargo:rerun-if-changed={}", metadata_path.display());
    let metadata: serde_json::Value = serde_json::from_slice(
        &fs::read(&metadata_path).expect("Windows application metadata is required"),
    )
    .expect("Windows application metadata must be valid JSON");
    let executable_name = metadata["executable_name"]
        .as_str()
        .filter(|value| !value.is_empty())
        .expect("Windows application metadata must provide executable_name");
    let icon_relative_path = metadata["icon"]
        .as_str()
        .filter(|value| !value.is_empty())
        .expect("Windows application metadata must provide icon");
    let internal_name = Path::new(executable_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .expect("Windows executable_name must have a valid file stem");
    let icon = root.join(icon_relative_path);
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-env-changed=RC");
    let output = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    let minor = env::var("CARGO_PKG_VERSION_MINOR").unwrap();
    let patch = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    let source = format!(
        r#"1 ICON "{}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},{build}
PRODUCTVERSION {major},{minor},{patch},{build}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0
FILEOS 0x40004L
FILETYPE 0x1L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Nauterm\0"
      VALUE "FileDescription", "{application_name}\0"
      VALUE "FileVersion", "{version}.{build}\0"
      VALUE "InternalName", "{internal_name}\0"
      VALUE "OriginalFilename", "{executable_name}\0"
      VALUE "ProductName", "{application_name}\0"
      VALUE "ProductVersion", "{version}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#,
        icon.display().to_string().replace('\\', "\\\\")
    );
    let input = output.join("nauterm-rs.rc");
    let resource = output.join("nauterm-rs.res");
    fs::write(&input, source).unwrap();
    let status = Command::new(env::var_os("RC").unwrap_or_else(|| "rc.exe".into()))
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg(input)
        .status()
        .expect("Windows SDK rc.exe is required; use a Visual Studio developer environment");
    assert!(status.success(), "Windows resource compilation failed");
    println!("cargo:rustc-link-arg-bin=nauterm-rs={}", resource.display());
}
