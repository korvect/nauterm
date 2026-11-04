//! Opt-in native association inspection. Does not open the Nauterm store,
//! launch applications, change associations or create a display connection.
use anyhow::Result;
use serde_json::json;

pub(crate) fn open_with() -> Result<()> {
    let renderer = gpui_kit::SvgRenderer::new(std::sync::Arc::new(crate::assets::Assets));
    let mut reports = Vec::new();
    for name in [
        "nauterm-diagnostic.txt",
        "nauterm-diagnostic.md",
        "nauterm-diagnostic.rs",
        "nauterm-diagnostic.json",
        "nauterm-diagnostic.png",
        "nauterm-diagnostic.deb",
        "nauterm-diagnostic.torrent",
        "nauterm-diagnostic.zip",
        "nauterm-diagnostic.pdf",
    ] {
        let started = std::time::Instant::now();
        let apps = crate::external_application::list_for_name(name)?;
        let metadata_elapsed = started.elapsed();
        crate::external_application::preload_application_icons(&apps);
        let elapsed = started.elapsed();
        let mut entries = Vec::new();
        for app in apps {
            let editor = crate::editor_config::Editor::from(app.clone());
            editor.validate()?;
            let image = crate::external_application::cached_icon(&editor);
            let decoded = image
                .as_ref()
                .map(|image| image.to_image_data(renderer.clone()));
            let decode_error = decoded
                .as_ref()
                .and_then(|result| result.as_ref().err())
                .map(ToString::to_string);
            let visible_pixels = decoded
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(|image| image.as_bytes(0))
                .map(|bytes| {
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|pixel| pixel[3] != 0)
                        .count()
                });
            entries.push(json!({
                "name": app.name,
                "identifier": app.identifier,
                "desktop_or_executable_exists": app.path.is_file(),
                "is_default": app.is_default,
                "icon_cached": image.is_some(),
                "icon_format": image.as_ref().map(|image| format!("{:?}", image.format)),
                "icon_decode_error": decode_error,
                "icon_visible_pixels": visible_pixels,
            }));
        }
        reports.push(json!({"filename": name, "applications": entries, "metadata_ms": metadata_elapsed.as_secs_f64() * 1000., "catalog_with_icons_ms": elapsed.as_secs_f64() * 1000.}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "platform": std::env::consts::OS,
            "association_reports": reports,
        }))?
    );
    Ok(())
}
