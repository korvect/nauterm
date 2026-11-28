use super::*;
use ::windows::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, SelectObject,
    },
    Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    UI::WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON},
};

const SIZE: usize = 32;
struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: Extraction and SHGetFileInfo transfer an owned icon handle.
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}
struct Bitmap {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: Restore the previous selection before deleting our bitmap.
        unsafe {
            let _ = SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
fn render(icon: HICON, background: u8) -> Option<Vec<u8>> {
    // SAFETY: A compatible memory DC and a top-down 32-bit DIB of a fixed small
    // size. The pixels are copied before Bitmap releases the native resources.
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE as i32,
                biHeight: -(SIZE as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixels, None, 0) {
            Ok(bitmap) => bitmap,
            Err(_) => {
                let _ = DeleteDC(dc);
                return None;
            }
        };
        let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
        if previous.is_invalid() || previous.0 as isize == -1 {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            return None;
        }
        let surface = Bitmap {
            dc,
            bitmap,
            previous,
        };
        if pixels.is_null() {
            return None;
        }
        std::ptr::write_bytes(pixels.cast::<u8>(), background, SIZE * SIZE * 4);
        DrawIconEx(dc, 0, 0, icon, SIZE as i32, SIZE as i32, 0, None, DI_NORMAL).ok()?;
        if !GdiFlush().as_bool() {
            return None;
        }
        let result = std::slice::from_raw_parts(pixels.cast::<u8>(), SIZE * SIZE * 4).to_vec();
        drop(surface);
        Some(result)
    }
}
fn png(icon: Icon) -> Option<Vec<u8>> {
    let black = render(icon.0, 0)?;
    let white = render(icon.0, 255)?;
    let rgba = windows_identity::rgba_from_mattes(&black, &white)?;
    use image::ImageEncoder;
    let mut png = vec![];
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            &rgba,
            SIZE as u32,
            SIZE as u32,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(png)
}
pub(super) fn extract(path: &str, index: i32) -> Option<Vec<u8>> {
    // Packaged applications expose PRI resource references, not HICON files.
    // Let the Shell resolve the package's scale/language-qualified PNG path.
    if path.starts_with("@{") {
        let source = HSTRING::from(path);
        let mut buffer = vec![0; 32768];
        // SAFETY: The UTF-16 source and fixed-capacity output remain live.
        unsafe { SHLoadIndirectString(&source, &mut buffer, None) }.ok()?;
        let end = buffer.iter().position(|&unit| unit == 0)?;
        let resolved = PathBuf::from(String::from_utf16(&buffer[..end]).ok()?);
        if !resolved.is_absolute() || fs::metadata(&resolved).ok()?.len() > 2 * 1024 * 1024 {
            return None;
        }
        let bytes = fs::read(resolved).ok()?;
        let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(bytes)).ok()?;
        let image = image::DynamicImage::from_decoder(decoder).ok()?;
        let rgba = image.thumbnail(SIZE as u32, SIZE as u32).to_rgba8();
        use image::ImageEncoder;
        let mut png = vec![];
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
            .ok()?;
        return Some(png);
    }
    let path = HSTRING::from(path);
    let mut icon = HICON::default();
    // SAFETY: The native extraction writes at most one owned icon handle.
    unsafe {
        SHDefExtractIconW(&path, index, 0, Some(&mut icon), None, SIZE as u32)
            .ok()
            .ok()?;
    }
    if icon.is_invalid() {
        return None;
    }
    png(Icon(icon))
}
pub(super) fn file(path: &Path) -> Option<Vec<u8>> {
    if !path.is_absolute() {
        return None;
    }
    let path = HSTRING::from(path.as_os_str());
    let mut info = SHFILEINFOW::default();
    // SAFETY: The Shell writes into a correctly-sized output structure. The
    // returned icon is owned and released after PNG conversion.
    let result = unsafe {
        SHGetFileInfoW(
            &path,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };
    if result == 0 || info.hIcon.is_invalid() {
        return None;
    }
    png(Icon(info.hIcon))
}
