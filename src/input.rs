//! Key strokes from the firmware console.
//!
//! [`Keys`] prefers `EFI_SIMPLE_TEXT_INPUT_EX_PROTOCOL`, which reports the full
//! scan code, and falls back to `EFI_SIMPLE_TEXT_INPUT_PROTOCOL` for firmware
//! that publishes only the basic console input protocol.
//!
//! [`Keys::try_read`] never waits. The application polls the host protocol and
//! stalls between turns, so one place owns the idle delay.

use core::ptr::NonNull;

use forms::Input;
use hii_uefi::raw::Boot;
use uefi_raw::protocol::console::{
    InputKey, KeyData, SimpleTextInputExProtocol, SimpleTextInputProtocol,
};

use crate::keys;

/// A non-blocking source of form inputs.
pub struct Keys {
    extended: Option<NonNull<SimpleTextInputExProtocol>>,
    basic: Option<NonNull<SimpleTextInputProtocol>>,
}

impl Keys {
    /// Opens the inherited system console, including its extended interface.
    ///
    /// # Errors
    ///
    /// Returns a message when the system table or its console is unavailable.
    pub fn open(boot: Boot) -> Result<Self, String> {
        let raw = uefi::table::system_table_raw().ok_or("the system table is unavailable")?;
        // SAFETY: Startup initializes this pointer from the inherited system
        // table, which stays live until this application returns.
        let table = unsafe { raw.as_ref() };
        let extended = if table.stdin_handle.is_null() {
            None
        } else {
            // SAFETY: The system table names the live console-input handle.
            unsafe {
                boot.protocol::<SimpleTextInputExProtocol>(
                    table.stdin_handle,
                    &SimpleTextInputExProtocol::GUID,
                )
            }
        };
        let basic = NonNull::new(table.stdin);
        if extended.is_none() && basic.is_none() {
            return Err("the firmware publishes no console input protocol".to_string());
        }
        Ok(Self { extended, basic })
    }

    /// Returns the next key stroke and maps it, or `None` when no key waits.
    ///
    /// Key strokes that carry no form meaning are consumed and skipped.
    pub fn try_read(&mut self) -> Option<Input> {
        if let Some(extended) = self.extended {
            let mut data = KeyData::default();
            // SAFETY: The protocol belongs to the firmware and `data` is
            // writable.
            let status =
                unsafe { (extended.as_ref().read_key_stroke_ex)(extended.as_ptr(), &mut data) };
            if !status.is_error() {
                let character = char::from_u32(u32::from(data.key.unicode_char)).unwrap_or('\0');
                return keys::map(data.key.scan_code, character);
            }
        }
        let basic = self.basic?;
        let mut key = InputKey::default();
        // SAFETY: The protocol belongs to the firmware and `key` is writable.
        let status = unsafe { (basic.as_ref().read_key_stroke)(basic.as_ptr(), &mut key) };
        if status.is_error() {
            return None;
        }
        let character = char::from_u32(u32::from(key.unicode_char)).unwrap_or('\0');
        keys::map(key.scan_code, character)
    }
}
