//! Theme TOML parsing shared by runtime loading and the build script.
#[derive(Debug, PartialEq)]
pub struct ParsedTheme {
    pub name: String,
    pub dark: bool,
    // foreground, background, cursor, accent, selection, selection text,
    // cursor text, then normal/bright ANSI colors.
    pub colors: [u32; 23],
}
#[allow(dead_code)] // Runtime uses parse_user; build scripts/tests validate bundled files.
pub fn parse(id: &str, source: &str) -> Result<ParsedTheme, String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err("Invalid bundled theme ID".into());
    }
    let document: toml::Value = toml::from_str(source).map_err(|error| format!("{id}: {error}"))?;
    let string = |path: &str| -> Result<&str, String> {
        path.split('.')
            .try_fold(&document, |node, key| node.get(key))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{id}: missing string {path}"))
    };
    let name = string("name")?.to_owned();
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err(format!("{id}: invalid name"));
    }
    let dark = match string("type")? {
        "dark" => true,
        "light" => false,
        _ => return Err(format!("{id}: invalid type")),
    };
    let paths = [
        "primary.foreground",
        "primary.background",
        "cursor.cursor",
        "primary.accent",
        "selection.background",
        "selection.text",
        "cursor.text",
        "normal.black",
        "normal.red",
        "normal.green",
        "normal.yellow",
        "normal.blue",
        "normal.magenta",
        "normal.cyan",
        "normal.white",
        "bright.black",
        "bright.red",
        "bright.green",
        "bright.yellow",
        "bright.blue",
        "bright.magenta",
        "bright.cyan",
        "bright.white",
    ];
    let mut colors = [0; 23];
    for (index, path) in paths.iter().enumerate() {
        let value = string(&format!("colors.{path}"))?;
        let hex = value.strip_prefix('#').unwrap_or(value);
        if !hex.is_ascii() {
            return Err(format!("{id}: non-ASCII RGB {path}"));
        }
        let hex = if hex.len() == 8 && hex[..2].eq_ignore_ascii_case("ff") {
            &hex[2..]
        } else {
            hex
        };
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("{id}: invalid opaque RGB {path}"));
        }
        colors[index] =
            u32::from_str_radix(hex, 16).map_err(|error| format!("{id}: {path}: {error}"))?;
    }
    Ok(ParsedTheme { name, dark, colors })
}

/// User files use the same optional fields/default colors as Flutter. Bundled
/// resources retain strict build-time validation through `parse` above.
#[allow(dead_code)] // This file is also compiled by the build script.
pub fn parse_user(id: &str, source: &str, defaults: [u32; 23]) -> Result<ParsedTheme, String> {
    let document: toml::Value = toml::from_str(source).map_err(|error| format!("{id}: {error}"))?;
    let get = |path: &str| {
        path.split('.')
            .try_fold(&document, |node, key| node.get(key))
            .and_then(toml::Value::as_str)
    };
    let name = get("name")
        .filter(|name| !name.trim().is_empty() && !name.chars().any(char::is_control))
        .unwrap_or(id)
        .to_owned();
    let dark = get("type") == Some("dark");
    let paths = [
        "primary.foreground",
        "primary.background",
        "cursor.cursor",
        "primary.accent",
        "selection.background",
        "selection.text",
        "cursor.text",
        "normal.black",
        "normal.red",
        "normal.green",
        "normal.yellow",
        "normal.blue",
        "normal.magenta",
        "normal.cyan",
        "normal.white",
        "bright.black",
        "bright.red",
        "bright.green",
        "bright.yellow",
        "bright.blue",
        "bright.magenta",
        "bright.cyan",
        "bright.white",
    ];
    let mut colors = defaults;
    for (index, path) in paths.iter().enumerate() {
        if let Some(value) = get(&format!("colors.{path}")) {
            let value = value.trim();
            let hex = value.strip_prefix('#').unwrap_or(value);
            if matches!(hex.len(), 6 | 8)
                && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                && let Ok(color) = u32::from_str_radix(hex, 16)
            {
                // Terminal/native palettes are RGB, including for ARGB TOML.
                colors[index] = color & 0x00ff_ffff;
            }
        }
    }
    Ok(ParsedTheme { name, dark, colors })
}

#[cfg(test)]
mod tests {
    #[test]
    fn strict_resource_parser_rejects_missing_invalid_and_transparent_colors() {
        let source = include_str!("../../assets/themes/ayu-dark.toml");
        let parsed = super::parse("ayu-dark", source).unwrap();
        assert_eq!(parsed.name, "Ayu Dark");
        assert!(parsed.dark);
        assert_eq!(parsed.colors[1], 0x0b0e14);
        assert_eq!(parsed.colors[7], 0x1e232b);
        for changed in [
            source.replace("#0b0e14", "#zzzzzz"),
            source.replace("#0b0e14", "#800b0e14"),
            source.replace("type = \"dark\"", "type = \"unknown\""),
            source.replace("name = \"Ayu Dark\"", "name = \"\""),
        ] {
            assert!(super::parse("ayu-dark", &changed).is_err());
        }
        assert!(super::parse("../../user-theme", source).is_err());
        assert!(super::parse("ayu-dark", "name='Missing'\ntype='dark'").is_err());
    }
}
