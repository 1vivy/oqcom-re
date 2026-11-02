//! Key stroke mapping.
//!
//! The firmware reports a scan code and a Unicode character for each key
//! stroke. This module turns those pairs into [`Input`] values for the forms
//! session. It is pure, so the mapping is testable on the host.

use forms::Input;

/// The UEFI scan code of the up arrow and of the volume-up key.
const SCAN_UP: u16 = 0x01;
/// The UEFI scan code of the down arrow and of the volume-down key.
const SCAN_DOWN: u16 = 0x02;
/// The UEFI scan code of the right arrow.
const SCAN_RIGHT: u16 = 0x03;
/// The UEFI scan code of the left arrow.
const SCAN_LEFT: u16 = 0x04;
/// The UEFI scan code of the home key.
const SCAN_HOME: u16 = 0x05;
/// The UEFI scan code of the escape key.
const SCAN_ESCAPE: u16 = 0x17;
/// The scan code that Qualcomm `ButtonsDxe` reports for the power key.
const SCAN_POWER: u16 = 0x0102;

/// Maps a firmware key stroke to a form input.
///
/// Arrow scan codes win over the Unicode character. The Qualcomm volume keys
/// report the arrow scan codes, and the power key reports [`SCAN_POWER`], so
/// recovery works on a phone with no keyboard attached.
///
/// # Examples
///
/// ```
/// use forms::Input;
/// use oqcom_re::keys::map;
///
/// assert_eq!(map(0x01, '\0'), Some(Input::Up));
/// assert_eq!(map(0x0102, '\0'), Some(Input::Enter));
/// assert_eq!(map(0, 'a'), Some(Input::Char('a')));
/// assert_eq!(map(0, '\u{1}'), None);
/// ```
pub fn map(scan: u16, unicode: char) -> Option<Input> {
    match scan {
        SCAN_UP => return Some(Input::Up),
        SCAN_DOWN => return Some(Input::Down),
        SCAN_RIGHT => return Some(Input::Right),
        SCAN_LEFT => return Some(Input::Left),
        SCAN_HOME => return Some(Input::Home),
        SCAN_ESCAPE => return Some(Input::Back),
        SCAN_POWER => return Some(Input::Enter),
        _ => {}
    }
    match unicode {
        '\r' | '\n' => Some(Input::Enter),
        '\u{8}' | '\u{7f}' => Some(Input::Erase),
        c if c == ' ' || c.is_ascii_graphic() => Some(Input::Char(c)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::map;
    use forms::Input;

    #[test]
    fn arrows_and_enter() {
        assert_eq!(map(0x01, '\0'), Some(Input::Up));
        assert_eq!(map(0x02, '\0'), Some(Input::Down));
        assert_eq!(map(0x03, '\0'), Some(Input::Right));
        assert_eq!(map(0x04, '\0'), Some(Input::Left));
        assert_eq!(map(0x05, '\0'), Some(Input::Home));
        assert_eq!(map(0, '\r'), Some(Input::Enter));
        assert_eq!(map(0, '\n'), Some(Input::Enter));
    }

    #[test]
    fn escape_and_erase() {
        assert_eq!(map(0x17, '\0'), Some(Input::Back));
        assert_eq!(map(0, '\u{8}'), Some(Input::Erase));
    }

    #[test]
    fn phone_keys() {
        // Volume up and down report the arrow scan codes.
        assert_eq!(map(0x01, '\0'), Some(Input::Up));
        assert_eq!(map(0x02, '\0'), Some(Input::Down));
        // The power key acts as Enter.
        assert_eq!(map(0x0102, '\0'), Some(Input::Enter));
    }

    #[test]
    fn text_and_ignored_keys() {
        assert_eq!(map(0, '7'), Some(Input::Char('7')));
        assert_eq!(map(0, ' '), Some(Input::Char(' ')));
        assert_eq!(map(0, '\0'), None);
        assert_eq!(map(0, '\u{1b}'), None);
        assert_eq!(map(0, 'é'), None);
    }
}
