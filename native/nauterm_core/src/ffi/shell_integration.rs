use std::ffi::c_char;
use std::io;
use std::path::{Path, PathBuf};

use super::common::{guard, string_from_ptr};

#[no_mangle]
pub extern "C" fn nauterm_shell_integration_write_resources(data_directory: *const c_char) -> bool {
    guard(false, || {
        let Some(data_directory) = string_from_ptr(data_directory) else {
            return false;
        };
        write_shell_integration_resources(Path::new(&data_directory)).is_ok()
    })
}

pub(crate) fn write_shell_integration_resources(data_directory: &Path) -> io::Result<PathBuf> {
    crate::shell_integration::write_resources(data_directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_stable_shell_integration_resources_idempotently() {
        let root = std::env::temp_dir().join(format!(
            "nauterm-shell-resource-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        let resources = write_shell_integration_resources(&root).unwrap();
        write_shell_integration_resources(&root).unwrap();

        assert_eq!(
            std::fs::read_to_string(resources.join("nauterm.zsh")).unwrap(),
            crate::pty::ZSH_INTEGRATION
        );
        assert_eq!(
            std::fs::read_to_string(resources.join("nauterm.bash")).unwrap(),
            crate::pty::BASH_INTEGRATION
        );
        assert_eq!(
            std::fs::read_to_string(resources.join("nauterm.fish")).unwrap(),
            crate::pty::FISH_INTEGRATION
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
