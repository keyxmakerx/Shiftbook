//! Helpers for the desktop crate's tests.

use std::path::PathBuf;

/// A temp folder removed on drop.
pub(crate) struct TempDir(pub(crate) PathBuf);

impl TempDir {
    pub(crate) fn new(label: &str) -> Self {
        let mut id = [0u8; 8];
        unique_bytes(&mut id);
        let path = std::env::temp_dir().join(format!(
            "shiftbook-desktop-{label}-{}",
            shiftbook_storage::hex::encode(&id)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_bytes(buf: &mut [u8]) {
    // Unique enough for a temp folder name; no extra crate needed.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mixed = nanos ^ (u128::from(std::process::id()) << 64);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (mixed >> (i * 8)) as u8;
    }
}

pub(crate) fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}
