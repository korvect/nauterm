//! GPUI's memory font collection alone is invisible to CoreText's named cascade.
//! Register the embedded symbols for this process before any line is shaped.
use anyhow::{Context, Result};
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_graphics::{CGDataProvider, CGFont};
use objc2_core_text::CTFontManagerError;
use std::ptr::{self, NonNull};

pub(super) fn register(bytes: &[u8]) -> Result<()> {
    let data = CFData::from_bytes(bytes);
    let provider = CGDataProvider::with_cf_data(Some(&data))
        .context("Could not create the bundled font data provider")?;
    let font = CGFont::with_data_provider(&provider).context("Could not decode bundled symbols")?;
    let mut error = ptr::null_mut();
    // Synchronous in-memory registration must finish before GPUI resolves its
    // first cascade. CoreText's newer descriptor registration is asynchronous.
    // SAFETY: the graphics font is retained and error is a valid out pointer.
    #[allow(deprecated)]
    let registered =
        unsafe { objc2_core_text::CTFontManagerRegisterGraphicsFont(&font, &mut error) };
    // SAFETY: CoreText returns an owned CFError on failure, or null.
    let error = NonNull::new(error).map(|error| unsafe { CFRetained::from_raw(error) });
    if registered
        || error
            .as_ref()
            .is_some_and(|error| error.code() == CTFontManagerError::AlreadyRegistered.0)
    {
        return Ok(());
    }
    anyhow::bail!(
        "Could not register bundled terminal symbols: {}",
        error
            .and_then(|error| error.description())
            .map(|description| description.to_string())
            .unwrap_or_else(|| "CoreText rejected the font".into())
    )
}
