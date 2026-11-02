//! The recovery application.
//!
//! This program is a `std` UEFI application. It lists every HII formset in the
//! HII database, shows the formsets with the shared forms session, writes
//! changed values back through the HII configuration protocols and, when it
//! leaves, returns to the caller.
//!
//! # Startup
//!
//! The program follows the `uefi-rs` standard-library example: it reads the
//! system table and image handle from [`std::os::uefi::env`] and hands them to
//! the `uefi` crate before it uses that crate. See
//! <https://rust-osdev.github.io/uefi-rs/how_to/rust-std.html>.
//!
//! `std::os::uefi::env` is behind the unstable `uefi_std` feature gate
//! (rust-lang/rust#100499), so this repository pins a nightly toolchain.
//!
//! # Markers
//!
//! The program writes these lines to the serial port. It also writes them to the
//! console, except while the session draws on the screen:
//!
//! - `OQCOM_RE_FORMSET title=<title>` for each formset that it found.
//! - `OQCOM_RE_READY formsets=<n>` after the first frame is drawn.
//! - `OQCOM_RE_EXIT` when it leaves.
//! - `OQCOM_RE_ERROR reason=<reason>` when a step fails.
//!
//! # Keys
//!
//! Up, down, left, right, home, enter, escape and backspace come from the
//! console. The Qualcomm volume keys report the arrow scan codes and the power
//! key reports scan code `0x0102`, which acts as enter.
#![cfg_attr(target_os = "uefi", feature(uefi_std))]

fn main() {
    #[cfg(target_os = "uefi")]
    app::main();
}

/// The freestanding UTF-16 `wcslen` libcall that the nightly standard library
/// emits on UEFI (`std/src/sys/backtrace.rs`, `std/src/panic.rs`).
///
/// The UEFI targets use a 16-bit `wchar_t` and have no C runtime that supplies
/// `wcslen`. The gobbl recovery application defines the same symbol.
///
/// # Safety
///
/// `text` must point to an aligned, readable, NUL-terminated UTF-16 sequence.
#[cfg(target_os = "uefi")]
#[unsafe(no_mangle)]
unsafe extern "C" fn wcslen(text: *const u16) -> usize {
    let mut length = 0;
    // SAFETY: The caller guarantees readable code units through the terminator.
    // The volatile read keeps LLVM from turning this loop into a call to itself.
    while unsafe { text.add(length).read_volatile() } != 0 {
        length += 1;
    }
    length
}

#[cfg(target_os = "uefi")]
mod app {
    use std::ptr::NonNull;

    use forms::{Event, Provider, Response, Session, View};
    use forms_tui::FormsWidget;
    use hii_uefi::hii::HiiProvider;
    use hii_uefi::raw::Boot;
    use oqcom_re::input::Keys;
    use oqcom_re::terminal::TextBackend;
    use ratatui_core::terminal::Terminal;
    use ratatui_gop::GopBackend;
    use uefi::boot::ScopedProtocol;
    use uefi::proto::console::gop::GraphicsOutput;
    use uefi::proto::console::text::Output;
    use uefi_raw::protocol::console::serial::SerialIoProtocol;
    use uefi_raw::table::system::SystemTable;

    /// The application entry point. StartImage returns when this returns.
    pub fn main() {
        let table = std::os::uefi::env::system_table();
        let image = std::os::uefi::env::image_handle();
        // SAFETY: The standard library keeps the system table and image handle
        // mapped until the program returns.
        unsafe {
            uefi::table::set_system_table(table.as_ptr().cast::<SystemTable>());
            if let Some(handle) = uefi::Handle::from_ptr(image.as_ptr()) {
                uefi::boot::set_image_handle(handle);
            }
        }
        // SAFETY: The standard library keeps the system table mapped until the
        // program returns, so the boot services outlive this call.
        let boot = unsafe { Boot::from_system_table(table.cast::<SystemTable>()) };
        let Some(boot) = boot else {
            println!("OQCOM_RE_ERROR reason=no boot services");
            return;
        };
        let log = Log::new(boot);
        if let Err(reason) = run(boot, &log) {
            log.line(&format!("OQCOM_RE_ERROR reason={reason}"));
        }
    }

    /// Lists the formsets, draws them and handles key strokes.
    fn run(boot: Boot, log: &Log) -> Result<(), String> {
        let mut provider =
            HiiProvider::new(boot).map_err(|error| format!("cannot open HII: {}", error.reason))?;
        let sets = provider
            .formsets()
            .map_err(|error| format!("cannot list formsets: {}", error.reason))?;
        for (_, set) in &sets {
            log.line(&format!("OQCOM_RE_FORMSET title={}", set.title));
        }
        for problem in provider.take_problems() {
            log.line(&format!("OQCOM_RE_ERROR reason={problem}"));
        }
        let count = sets.len();
        let mut session = Session::new(sets);
        if let Some(guid) = load_options_formset()? {
            if !session.enter(&guid) {
                // A caller that names one formset wants that question, not the list.
                log.line("OQCOM_RE_EXIT");
                return Err(format!("formset {} is not published", guid_text(&guid)));
            }
            log.line(&format!("OQCOM_RE_FOCUS formset={}", guid_text(&guid)));
        }
        let mut drawing = Drawing::open()?;
        // From here the session owns the screen: markers go to the serial port only.
        log.console.set(false);
        log.line(&format!("OQCOM_RE_DISPLAY backend={}", drawing.name()));
        let mut keys = Keys::open(boot)?;
        let view = session.view(&mut provider);
        drawing.draw(&view)?;
        log.line(&format!("OQCOM_RE_READY formsets={count}"));
        loop {
            let input = keys.read();
            match session.input(input, &mut provider) {
                Event::Exit => break,
                Event::Failed(error) => {
                    log.line(&format!("OQCOM_RE_ERROR reason={}", error.reason))
                }
                Event::Changed | Event::Response(Response::Refresh) => match provider.formsets() {
                    Ok(sets) => {
                        session.replace(sets);
                        for problem in provider.take_problems() {
                            log.line(&format!("OQCOM_RE_ERROR reason={problem}"));
                        }
                    }
                    Err(error) => {
                        log.line(&format!("OQCOM_RE_ERROR reason={}", error.reason));
                    }
                },
                Event::None | Event::Response(_) => {}
            }
            let view = session.view(&mut provider);
            drawing.draw(&view)?;
        }
        log.console.set(true);
        log.line("OQCOM_RE_EXIT");
        Ok(())
    }

    /// Reads `--formset <GUID>` from this image's load options.
    fn load_options_formset() -> Result<Option<forms::Guid>, String> {
        let image = uefi::boot::open_protocol_exclusive::<uefi::proto::loaded_image::LoadedImage>(
            uefi::boot::image_handle(),
        )
        .map_err(|error| format!("cannot open LoadedImage: {error:?}"))?;
        // Absent or non-UTF-16 load options name no formset.
        let Ok(options) = image.load_options_as_cstr16() else {
            return Ok(None);
        };
        oqcom_re::options::formset(&options.to_string()).map_err(|error| format!("{error:?}"))
    }

    /// Formats a GUID in registry form from its UEFI byte order.
    fn guid_text(guid: &forms::Guid) -> String {
        let a = u32::from_le_bytes([guid[0], guid[1], guid[2], guid[3]]);
        let b = u16::from_le_bytes([guid[4], guid[5]]);
        let c = u16::from_le_bytes([guid[6], guid[7]]);
        let tail: String = guid[10..]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!(
            "{a:08x}-{b:04x}-{c:04x}-{:02x}{:02x}-{tail}",
            guid[8], guid[9]
        )
    }

    /// The drawing surface: GOP when available, else the text console.
    enum Drawing {
        /// A framebuffer from `EFI_GRAPHICS_OUTPUT_PROTOCOL`.
        Gop(Terminal<GopBackend>),
        /// The text console from `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`.
        Text(Terminal<TextBackend>),
    }

    impl Drawing {
        /// Opens the best available drawing surface.
        fn open() -> Result<Self, String> {
            if let Some(terminal) = Self::gop()? {
                return Ok(Self::Gop(terminal));
            }
            let output = open_text()?;
            let backend = TextBackend::new(output);
            let terminal =
                Terminal::new(backend).map_err(|error| format!("no terminal: {error}"))?;
            Ok(Self::Text(terminal))
        }

        /// Returns the name of the drawing surface, for the log.
        fn name(&self) -> &'static str {
            match self {
                Self::Gop(_) => "gop",
                Self::Text(_) => "text",
            }
        }

        /// Opens the graphics output protocol, when the firmware has one.
        fn gop() -> Result<Option<Terminal<GopBackend>>, String> {
            let Ok(handle) = uefi::boot::get_handle_for_protocol::<GraphicsOutput>() else {
                return Ok(None);
            };
            let Ok(gop) = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(handle) else {
                return Ok(None);
            };
            let Ok(backend) = GopBackend::new(gop, None) else {
                return Ok(None);
            };
            let terminal =
                Terminal::new(backend).map_err(|error| format!("no terminal: {error}"))?;
            Ok(Some(terminal))
        }

        /// Draws one view.
        fn draw(&mut self, view: &View) -> Result<(), String> {
            let paint = |frame: &mut ratatui_core::terminal::Frame| {
                frame.render_widget(FormsWidget::new(view), frame.area());
            };
            match self {
                Self::Gop(terminal) => terminal
                    .draw(paint)
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                Self::Text(terminal) => terminal
                    .draw(paint)
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
            }
        }
    }

    /// Opens the firmware text console.
    fn open_text() -> Result<ScopedProtocol<Output>, String> {
        let handle = uefi::boot::get_handle_for_protocol::<Output>()
            .map_err(|error| format!("no text console: {error}"))?;
        uefi::boot::open_protocol_exclusive::<Output>(handle)
            .map_err(|error| format!("no text console: {error}"))
    }

    /// Marker output to the serial port and, while no session draws, the console.
    struct Log {
        serial: Option<NonNull<SerialIoProtocol>>,
        /// The console receives markers only while the session does not own the screen.
        console: std::cell::Cell<bool>,
    }

    impl Log {
        /// Finds the serial port, when the firmware has one.
        fn new(boot: Boot) -> Self {
            Self {
                serial: boot.locate::<SerialIoProtocol>(&SerialIoProtocol::GUID),
                console: std::cell::Cell::new(true),
            }
        }

        /// Writes one line to the serial port, and to the console when it is free.
        fn line(&self, text: &str) {
            let mut line = String::from(text);
            line.push_str("\r\n");
            if let Some(serial) = self.serial {
                let mut size = line.len();
                // SAFETY: The protocol belongs to the firmware and the buffer
                // stays alive for the call.
                unsafe {
                    let _ = (serial.as_ref().write)(serial.as_ptr(), &mut size, line.as_ptr());
                }
            }
            if self.console.get() {
                print!("{line}");
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }
    }
}
