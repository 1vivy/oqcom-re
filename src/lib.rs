//! The recovery environment's own modules.
//!
//! The binary lists every HII formset and shows it with the shared forms
//! session; the provider, codec and widget come from the `forms` repository.
//! This library holds the parts that only this application needs:
//!
//! - [`keys`] maps firmware key strokes to form inputs.
//! - [`options`] reads `--formset <GUID>` from the load options.
//! - [`input`] reads those key strokes from the console.
//! - [`host`] polls the optional protocol that the caller published.
//! - [`terminal`] draws with the firmware text console when there is no GOP.
//!
//! Modules that talk to firmware exist on UEFI targets only.

pub mod keys;
pub mod options;

#[cfg(target_os = "uefi")]
pub mod host;
#[cfg(target_os = "uefi")]
pub mod input;
#[cfg(target_os = "uefi")]
pub mod terminal;
