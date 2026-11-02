//! Load options of the application.
//!
//! The caller of `StartImage` may name one formset in the image's load options:
//! `--formset <GUID>`. The application then opens that formset directly and ends
//! when the person leaves its entry form, as `FormBrowser2.SendForm` does. A
//! firmware owner uses this for one question, for example a consent prompt.
//!
//! Other tokens are ignored, because a shell passes the image path first.

use forms::Guid;

/// A load-option failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionError {
    /// `--formset` is the last token.
    MissingFormset,
    /// The `--formset` value is not a GUID in registry format.
    BadGuid(String),
}

/// Returns the formset that the load options name, if any.
///
/// # Errors
///
/// Returns [`OptionError`] when `--formset` has no value or a value that is not
/// a GUID in `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` form.
///
/// # Examples
///
/// ```
/// use oqcom_re::options::formset;
///
/// let guid = formset("oqcom-re.efi --formset 00112233-4455-6677-8899-aabbccddeeff").unwrap();
/// assert_eq!(guid.map(|g| g[0]), Some(0x33));
/// assert_eq!(formset("oqcom-re.efi").unwrap(), None);
/// ```
pub fn formset(options: &str) -> Result<Option<Guid>, OptionError> {
    let mut tokens = options.split_whitespace();
    while let Some(token) = tokens.next() {
        if token == "--formset" {
            let value = tokens.next().ok_or(OptionError::MissingFormset)?;
            return parse_guid(value)
                .map(Some)
                .ok_or_else(|| OptionError::BadGuid(value.into()));
        }
    }
    Ok(None)
}

/// Parses a registry-format GUID into the UEFI byte order.
///
/// The first three fields are little-endian in an `EFI_GUID`; the last eight
/// bytes keep their text order.
fn parse_guid(text: &str) -> Option<Guid> {
    let fields: Vec<&str> = text.split('-').collect();
    let [a, b, c, d, e] = fields.as_slice() else {
        return None;
    };
    if a.len() != 8 || b.len() != 4 || c.len() != 4 || d.len() != 4 || e.len() != 12 {
        return None;
    }
    let mut guid = [0; 16];
    guid[..4].copy_from_slice(&u32::from_str_radix(a, 16).ok()?.to_le_bytes());
    guid[4..6].copy_from_slice(&u16::from_str_radix(b, 16).ok()?.to_le_bytes());
    guid[6..8].copy_from_slice(&u16::from_str_radix(c, 16).ok()?.to_le_bytes());
    let tail = [*d, *e].concat();
    for (index, byte) in guid[8..].iter_mut().enumerate() {
        *byte = u8::from_str_radix(tail.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(guid)
}

#[cfg(test)]
mod tests {
    use super::{OptionError, formset};

    #[test]
    fn guid_uses_the_uefi_byte_order() {
        let guid = formset("--formset 00112233-4455-6677-8899-aabbccddeeff").unwrap();
        assert_eq!(
            guid,
            Some([
                0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff
            ])
        );
    }

    #[test]
    fn other_tokens_are_ignored_and_absence_is_not_an_error() {
        assert_eq!(formset(""), Ok(None));
        assert_eq!(formset("\\EFI\\ODP\\RE.EFI -v"), Ok(None));
    }

    #[test]
    fn malformed_values_are_refused() {
        assert_eq!(formset("--formset"), Err(OptionError::MissingFormset));
        for bad in [
            "0011223-4455-6677-8899-aabbccddeeff",
            "00112233-4455-6677-8899-aabbccddeefg",
            "00112233445566778899aabbccddeeff",
            "00112233-4455-6677-8899aabb-ccddeeff",
        ] {
            assert_eq!(
                formset(&format!("--formset {bad}")),
                Err(OptionError::BadGuid(bad.into()))
            );
        }
    }
}
