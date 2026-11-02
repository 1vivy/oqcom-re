//! The optional cooperative host protocol.
//!
//! A caller that starts this application can publish
//! [`oqcom_re_host::Protocol`] first. [`Host`] looks that interface up once at
//! startup, checks its revision and polls it at the top of every input-loop
//! turn. The application knows nothing about the host transport: it only reads
//! the request word and returns when the caller asks for the screen.
//!
//! Without the protocol the application runs alone, exactly as before.

use core::ptr::NonNull;

use hii_uefi::raw::Boot;
use oqcom_re_host::{PROTOCOL_GUID, Protocol, REVISION};
use uefi_raw::Guid;

pub use oqcom_re_host::Disposition;

/// The host protocol interface that this caller published.
pub struct Host {
    /// The firmware-owned interface. The caller installs it before
    /// `StartImage` and keeps it valid for the application lifetime; it may
    /// stay registered after the application returns.
    protocol: NonNull<Protocol>,
}

impl Host {
    /// Locates the protocol and checks its revision.
    ///
    /// Returns `Ok(None)` when the caller published no protocol. That is the
    /// standalone case, and the application then runs its own session.
    ///
    /// # Errors
    ///
    /// Returns a message when the interface revision is not [`REVISION`].
    /// The caller must not use an interface that it does not understand.
    pub fn locate(boot: Boot) -> Result<Option<Self>, String> {
        let guid = Guid::from_bytes(PROTOCOL_GUID);
        let Some(protocol) = boot.locate::<Protocol>(&guid) else {
            return Ok(None);
        };
        // SAFETY: `locate` returned a live interface, and the publisher keeps
        // it alive until this application returns from `StartImage`.
        let revision = unsafe { protocol.as_ref().revision };
        if revision != REVISION {
            return Err(format!(
                "host protocol revision {revision} is not {REVISION}"
            ));
        }
        Ok(Some(Self { protocol }))
    }

    /// Polls the host transport once and classifies its request.
    ///
    /// # Errors
    ///
    /// Returns a message with the actual status when the publisher reports a
    /// failure. The application then ends the session instead of claiming a
    /// host access that it does not have.
    pub fn poll(&mut self) -> Result<Disposition, String> {
        let this = self.protocol.as_ptr();
        let mut request = 0u32;
        // SAFETY: The publisher guarantees one interface for the application
        // lifetime, a poll that runs at `TPL_APPLICATION`, and a request word
        // that it writes on every path. `this` and `request` stay valid for
        // the call.
        let status = unsafe { ((*this).poll)(this, &mut request) };
        oqcom_re_host::disposition(status, request)
            .map_err(|status| format!("host poll failed: status {status:#x}"))
    }
}
