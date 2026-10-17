// Deleting all data needs a typed word. The app checks it again in Rust
// (shiftbook_core::wipe); this only decides when the button turns on.

/** The word to type (matches DELETE_CONFIRM_WORD in crates/core). */
export const DELETE_CONFIRM_WORD = 'DELETE';

/** True when `typed` is the word, ignoring case and spaces around it. */
export function confirmsDelete(typed: string): boolean {
  return typed.trim().toUpperCase() === DELETE_CONFIRM_WORD;
}
