use std::{env, fs, path::PathBuf};
#[path = "src/theme_resource_parser.rs"]
mod theme_resource_parser;
#[path = "build_support/windows_resources.rs"]
mod windows_resources;

fn main() {
    for name in [
        "NAUTERM_GITHUB_CLIENT_ID",
        "NAUTERM_GOOGLE_CLIENT_ID",
        "NAUTERM_GOOGLE_CLIENT_SECRET",
        "NAUTERM_ONEDRIVE_CLIENT_ID",
        "NAUTERM_DROPBOX_CLIENT_ID",
        "NAUTERM_UPDATE_REPOSITORY",
        "NAUTERM_BUILD_NUMBER",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let build = env::var("NAUTERM_BUILD_NUMBER").unwrap_or_default();
    let build = if build.is_empty() {
        1
    } else {
        assert!(
            build.len() <= 5 && build.bytes().all(|byte| byte.is_ascii_digit()),
            "NAUTERM_BUILD_NUMBER must be an integer from 1 through 65535"
        );
        build
            .parse::<u16>()
            .ok()
            .filter(|number| *number > 0)
            .expect("NAUTERM_BUILD_NUMBER must be an integer from 1 through 65535")
    };
    println!("cargo:rustc-env=NAUTERM_RS_BUILD_NUMBER={build}");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    windows_resources::compile(&root, build);
    generate_host_icons(&root);
    generate(root);
}

fn generate_host_icons(root: &std::path::Path) {
    let directory = root.join("../assets/icons/os");
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut icons = vec![];
    for entry in fs::read_dir(&directory).expect("Reference OS icons are required") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "svg") {
            continue;
        }
        assert!(
            entry.file_type().unwrap().is_file(),
            "Expected an ordinary SVG resource"
        );
        let name = entry.file_name().into_string().unwrap();
        icons.push((format!("nauterm/os/{name}"), path.canonicalize().unwrap()));
    }
    icons.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!icons.is_empty());
    let mut source = String::from("pub static HOST_ICONS: &[(&str, &[u8])] = &[\n");
    for (name, path) in icons {
        source.push_str(&format!(
            "({name:?}, include_bytes!({:?})),\n",
            path.to_str().unwrap()
        ));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("host_icons.rs"),
        source,
    )
    .unwrap();
}

pub fn generate(root: PathBuf) {
    let resources = root.join("../assets/themes");
    println!("cargo:rerun-if-changed={}", resources.display());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("src/theme_resource_parser.rs").display()
    );
    let mut records = vec![];
    for entry in fs::read_dir(&resources).expect("Reference theme resources are required") {
        let entry = entry.unwrap();
        assert!(
            !entry.file_type().unwrap().is_symlink(),
            "Unexpected theme resource symlink"
        );
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let id = path.file_stem().unwrap().to_str().unwrap().to_owned();
        assert!(
            !matches!(id.as_str(), "nysa-light" | "nysa-dark"),
            "Built-in theme ID collision"
        );
        let theme = theme_resource_parser::parse(&id, &fs::read_to_string(path).unwrap()).unwrap();
        records.push((id, theme));
    }
    records.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(!records.is_empty() && records.len() < usize::from(u16::MAX) - 2);
    let mut generated = String::from(
        "pub static THEMES: &[Theme] = &[\nTheme { id: \"nysa-light\", name: \"Nysa Light\", palette: Palette::nysa_light() },\nTheme { id: \"nysa-dark\", name: \"Nysa Dark\", palette: Palette::nysa_dark() },\n",
    );
    for (id, theme) in records {
        let c = theme.colors;
        generated.push_str(&format!("Theme {{ id: {id:?}, name: {:?}, palette: Palette {{ base: BaseColors {{ foreground: {}, background: {}, cursor: {} }}, accent: {}, selection: {}, selection_text: {}, cursor_text: {}, dark: {}, ansi: {:?} }} }},\n", theme.name, c[0], c[1], c[2], c[3], c[4], c[5], c[6], theme.dark, &c[7..]));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("terminal_themes.rs"),
        generated,
    )
    .unwrap();
}
