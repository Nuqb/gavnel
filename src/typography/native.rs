//! Private GDI resources live until process exit, like the shared egui font bytes.
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use windows::Win32::Graphics::Gdi::AddFontMemResourceEx;

// Keep handles for process lifetime; Windows releases these private resources on
// exit. Storing the opaque value avoids sharing dereferenceable pointers.
static REGISTERED: LazyLock<Mutex<HashMap<&'static str, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn ensure(name: &'static str, bytes: &[u8]) -> bool {
    let mut resources = REGISTERED.lock().unwrap();
    let handle = resources.entry(name).or_insert_with(|| {
        let mut count = 0;
        // Windows metadata declares this output pointer const; keep mutable
        // storage and pass a raw pointer because the API writes the face count.
        // Windows copies the immutable font data. Registration is process-private
        // and does not write files, registry settings, or system font settings.
        unsafe {
            AddFontMemResourceEx(
                bytes.as_ptr().cast(),
                bytes.len() as u32,
                None,
                &raw mut count,
            )
            .0 as usize
        }
    });
    *handle != 0
}
