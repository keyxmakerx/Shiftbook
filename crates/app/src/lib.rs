//! Shiftbook's app layer: puts records from `shiftbook-storage` together into the views the
//! UI shows, using the rules in `shiftbook-core`. It has no transport: the desktop app's
//! Tauri commands call it now, and the Phase 2 server's HTTP handlers will call the same
//! functions, so both always show the same thing.
#![forbid(unsafe_code)]

pub mod desk;
pub mod views;
