//! The cooperative host protocol that `oqcom-re` polls while it runs.
//!
//! A caller can publish [`Protocol`] under [`PROTOCOL_GUID`] before it starts
//! the recovery application. The application looks the protocol up once at
//! startup and polls it at the top of every input-loop turn. The protocol lets
//! the caller take the screen back for its own host transport, for example a
//! native fastboot session, without a second screen owner.
//!
//! # Contract
//!
//! - The publisher installs one [`Protocol`] interface and keeps it alive
//!   until the application has returned from `StartImage`.
//! - [`Protocol::poll`] runs at `TPL_APPLICATION` only. It never runs from a
//!   timer notification callback.
//! - [`Protocol::poll`] writes the request word on every supported path, also
//!   when it returns an error.
//! - A request of [`YIELD_TO_HOST`] means the application returns to its
//!   caller at once, without acting on the selected form row.
//! - Zero and unknown request values mean the application keeps the screen.
//! - A status other than [`SUCCESS`], or an interface revision other than
//!   [`REVISION`], is reported by the application and ends the session. The
//!   application never claims access that the publisher did not grant.
//!
//! The crate has no dependencies and compiles on stable Rust 1.95, because a
//! firmware project adds it as a git dependency.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs, unsafe_op_in_unsafe_fn)]
#![deny(clippy::all, clippy::undocumented_unsafe_blocks)]

/// The protocol GUID `34bf6d61-c86c-4e92-92b6-5937d468ca2d` in UEFI byte order.
pub const PROTOCOL_GUID: [u8; 16] = [
    0x61, 0x6d, 0xbf, 0x34, 0x6c, 0xc8, 0x92, 0x4e, 0x92, 0xb6, 0x59, 0x37, 0xd4, 0x68, 0xca, 0x2d,
];

/// The protocol revision that this crate describes.
pub const REVISION: u64 = 1;

/// The request that makes the application return to its caller at once.
pub const YIELD_TO_HOST: u32 = 1;

/// The `EFI_STATUS` value that reports success.
pub const SUCCESS: usize = 0;

/// The interface that a caller publishes under [`PROTOCOL_GUID`].
///
/// The layout is `#[repr(C)]` and matches the UEFI calling convention on every
/// firmware architecture that the application supports.
#[repr(C)]
pub struct Protocol {
    /// The interface revision. A publisher sets this to [`REVISION`].
    pub revision: u64,
    /// Polls the host transport once.
    ///
    /// `this` is the interface that the application located. The publisher
    /// writes the request word through `request`; the field is always written,
    /// also on an error path. The return value is an `EFI_STATUS` in native
    /// `usize` form.
    ///
    /// The publisher calls this function only at `TPL_APPLICATION` and never
    /// from a timer notification. A publisher that has no work to offer writes
    /// zero and returns [`SUCCESS`].
    pub poll: unsafe extern "efiapi" fn(this: *mut Protocol, request: *mut u32) -> usize,
}

/// What the application does after one poll.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    /// The application keeps the screen and reads the next key stroke.
    Idle,
    /// The application returns to its caller at once.
    Yield,
}

/// Classifies one poll result.
///
/// The application passes the request word and the `EFI_STATUS` that the
/// publisher returned. A non-zero status is returned as `Err` so the caller
/// can report the actual value.
///
/// # Examples
///
/// ```
/// use oqcom_re_host::{disposition, Disposition, SUCCESS, YIELD_TO_HOST};
///
/// assert_eq!(disposition(SUCCESS, 0), Ok(Disposition::Idle));
/// assert_eq!(disposition(SUCCESS, YIELD_TO_HOST), Ok(Disposition::Yield));
/// assert_eq!(disposition(SUCCESS, 2), Ok(Disposition::Idle));
/// assert_eq!(disposition(5, YIELD_TO_HOST), Err(5));
/// ```
pub fn disposition(status: usize, request: u32) -> Result<Disposition, usize> {
    if status != SUCCESS {
        return Err(status);
    }
    if request == YIELD_TO_HOST {
        Ok(Disposition::Yield)
    } else {
        Ok(Disposition::Idle)
    }
}

#[cfg(test)]
mod tests {
    use core::ptr;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Counts the callback calls of [`record_poll`].
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    /// An executable publisher callback: writes [`YIELD_TO_HOST`] every other
    /// call and returns success.
    unsafe extern "efiapi" fn record_poll(this: *mut Protocol, request: *mut u32) -> usize {
        assert!(!this.is_null());
        assert!(!request.is_null());
        // SAFETY: The caller supplies a writable request word, and the test
        // checks the pointer first.
        unsafe { ptr::write(request, YIELD_TO_HOST) };
        CALLS.fetch_add(1, Ordering::SeqCst);
        // SAFETY: The test calls this function with its own interface.
        let revision = unsafe { (*this).revision };
        assert_eq!(revision, REVISION);
        SUCCESS
    }

    #[test]
    fn guid_bytes_follow_uefi_order() {
        assert_eq!(
            PROTOCOL_GUID,
            [
                0x61, 0x6d, 0xbf, 0x34, 0x6c, 0xc8, 0x92, 0x4e, 0x92, 0xb6, 0x59, 0x37, 0xd4, 0x68,
                0xca, 0x2d,
            ]
        );
    }

    #[test]
    fn interface_layout_is_stable() {
        assert_eq!(core::mem::size_of::<Protocol>(), 16);
        assert_eq!(core::mem::align_of::<Protocol>(), 8);
    }

    #[test]
    fn the_callback_runs_with_the_interface_and_a_request_word() {
        let mut protocol = Protocol {
            revision: REVISION,
            poll: record_poll,
        };
        let mut request = 0;
        // SAFETY: `protocol` and `request` are live for the call.
        let status = unsafe { (protocol.poll)(&mut protocol, &mut request) };
        assert_eq!(status, SUCCESS);
        assert_eq!(request, YIELD_TO_HOST);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(disposition(status, request), Ok(Disposition::Yield));
    }

    #[test]
    fn an_error_status_keeps_its_value() {
        assert_eq!(disposition(usize::MAX, YIELD_TO_HOST), Err(usize::MAX));
        assert_eq!(disposition(0x8000_0002, 0), Err(0x8000_0002));
    }

    #[test]
    fn unknown_requests_keep_the_screen() {
        assert_eq!(disposition(SUCCESS, 0), Ok(Disposition::Idle));
        assert_eq!(disposition(SUCCESS, 3), Ok(Disposition::Idle));
    }
}
