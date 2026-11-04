//! Native application-launcher file chooser. Cancellation returns no selection;
//! dropping the receiving task discards any result before an app can launch.
//! On Windows rfd owns the dialog's STA thread until the native dialog closes.
use super::*;
use std::{future::Future, pin::Pin};

pub struct Choice {
    picker: Pin<Box<dyn Future<Output = Option<rfd::FileHandle>>>>,
}
pub fn choose(window: &gpui_kit::Window) -> Result<Choice> {
    let picker = rfd::AsyncFileDialog::new().set_parent(window);
    #[cfg(target_os = "linux")]
    let picker = picker
        .set_directory("/usr/share/applications")
        .add_filter("Applications", &["desktop"]);
    #[cfg(target_os = "windows")]
    let picker = {
        let picker = picker.add_filter("Applications", &["exe", "com"]);
        if let Some(directory) =
            std::env::var_os("ProgramFiles").filter(|path| Path::new(path).is_absolute())
        {
            picker.set_directory(directory)
        } else {
            picker
        }
    };
    let picker = picker.pick_file();
    Ok(Choice {
        picker: Box::pin(picker),
    })
}
impl Choice {
    pub async fn receive(self) -> Result<Option<Application>> {
        let Some(file) = self.picker.await else {
            return Ok(None);
        };
        let path = file.path().to_owned();
        #[cfg(target_os = "linux")]
        {
            smol::unblock(move || linux::from_path(&path))
                .await
                .map(Some)
        }
        #[cfg(target_os = "windows")]
        {
            smol::unblock(move || windows::from_path(&path))
                .await
                .map(Some)
        }
    }
}
