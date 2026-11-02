//! A Ratatui backend over the firmware text console.
//!
//! The firmware text console has no colour depth worth speaking of and no
//! cursor addressing beyond `set_cursor_position`, so the backend writes every
//! changed cell at its own position. `ratatuefi` 0.1.1 does the same, but it
//! pins `uefi` 0.38 and enables that crate's global allocator and panic
//! handler, which a `std` program cannot use. This module keeps the approach
//! and the `uefi` 0.41 types of the rest of the program.

use core::fmt;

use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};
use ratatui_core::style::Color;
use uefi::CStr16;
use uefi::boot::ScopedProtocol;
use uefi::proto::console::text::{Color as TextColor, Output};

/// A drawing failure on the text console.
#[derive(Debug)]
pub struct TextError {
    /// The firmware status of the failed call.
    pub status: uefi::Status,
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "text console error: {}", self.status)
    }
}

impl core::error::Error for TextError {}

impl From<uefi::Error> for TextError {
    fn from(error: uefi::Error) -> Self {
        Self {
            status: error.status(),
        }
    }
}

/// Maps a Ratatui colour to a firmware foreground colour.
fn foreground(color: Color) -> TextColor {
    match color {
        Color::Black => TextColor::Black,
        Color::Red => TextColor::Red,
        Color::Green => TextColor::Green,
        Color::Yellow => TextColor::Yellow,
        Color::Blue => TextColor::Blue,
        Color::Magenta => TextColor::Magenta,
        Color::Cyan => TextColor::Cyan,
        Color::Gray => TextColor::LightGray,
        Color::DarkGray => TextColor::DarkGray,
        Color::LightRed => TextColor::LightRed,
        Color::LightGreen => TextColor::LightGreen,
        Color::LightYellow => TextColor::Yellow,
        Color::LightBlue => TextColor::LightBlue,
        Color::LightMagenta => TextColor::LightMagenta,
        Color::LightCyan => TextColor::LightCyan,
        Color::White => TextColor::White,
        Color::Reset | Color::Indexed(_) | Color::Rgb(..) => TextColor::LightGray,
    }
}

/// Maps a Ratatui colour to a firmware background colour.
///
/// The firmware accepts the first eight colours as backgrounds. Other colours
/// fall back to black so that text stays readable.
fn background(color: Color) -> TextColor {
    match color {
        Color::Black => TextColor::Black,
        Color::Red => TextColor::Red,
        Color::Green => TextColor::Green,
        Color::Yellow | Color::LightYellow => TextColor::Brown,
        Color::Blue => TextColor::Blue,
        Color::Magenta => TextColor::Magenta,
        Color::Cyan => TextColor::Cyan,
        Color::Gray | Color::White => TextColor::LightGray,
        _ => TextColor::Black,
    }
}

/// A Ratatui backend that writes to the firmware text console.
pub struct TextBackend {
    output: ScopedProtocol<Output>,
    scratch: Vec<u16>,
}

impl TextBackend {
    /// Wraps an opened text output protocol.
    pub fn new(output: ScopedProtocol<Output>) -> Self {
        Self {
            output,
            scratch: Vec::new(),
        }
    }

    /// Returns the size of the current text mode.
    fn columns_rows(&self) -> (u16, u16) {
        match self.output.current_mode() {
            Ok(Some(mode)) => (mode.columns() as u16, mode.rows() as u16),
            _ => (80, 25),
        }
    }

    /// Writes one cell at a position.
    fn write_cell(&mut self, x: u16, y: u16, cell: &Cell) -> Result<(), TextError> {
        self.output
            .set_cursor_position(usize::from(x), usize::from(y))?;
        self.output
            .set_color(foreground(cell.fg), background(cell.bg))?;
        self.scratch.clear();
        self.scratch.extend(cell.symbol().encode_utf16());
        self.scratch.push(0);
        let text = CStr16::from_u16_with_nul(&self.scratch).map_err(|_| TextError {
            status: uefi::Status::INVALID_PARAMETER,
        })?;
        self.output.output_string(text)?;
        Ok(())
    }

    /// Writes blanks over a range of one row.
    fn blank_row(&mut self, row: u16, first: u16, last: u16) -> Result<(), TextError> {
        let (columns, _) = self.columns_rows();
        let mut column = first.min(columns);
        let end = last.min(columns);
        while column < end {
            self.output
                .set_cursor_position(usize::from(column), usize::from(row))?;
            self.output
                .set_color(TextColor::LightGray, TextColor::Black)?;
            let blank = CStr16::from_u16_with_nul(&[0x20, 0]).map_err(|_| TextError {
                status: uefi::Status::INVALID_PARAMETER,
            })?;
            self.output.output_string(blank)?;
            column += 1;
        }
        Ok(())
    }
}

impl Backend for TextBackend {
    type Error = TextError;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            self.write_cell(x, y, cell)?;
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        let _ = self.output.enable_cursor(false);
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        let _ = self.output.enable_cursor(true);
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        let (column, row) = self.output.cursor_position();
        Ok(Position {
            x: column as u16,
            y: row as u16,
        })
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        self.output
            .set_cursor_position(usize::from(position.x), usize::from(position.y))?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.output.clear()?;
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        let (columns, rows) = self.columns_rows();
        let (column, row) = self.output.cursor_position();
        let (column, row) = (column as u16, row as u16);
        match clear_type {
            ClearType::All => self.clear(),
            ClearType::CurrentLine => self.blank_row(row, 0, columns),
            ClearType::UntilNewLine => self.blank_row(row, column, columns),
            ClearType::BeforeCursor => {
                for line in 0..row {
                    self.blank_row(line, 0, columns)?;
                }
                self.blank_row(row, 0, column)
            }
            ClearType::AfterCursor => {
                self.blank_row(row, column, columns)?;
                for line in row + 1..rows {
                    self.blank_row(line, 0, columns)?;
                }
                Ok(())
            }
        }
    }

    fn size(&self) -> Result<Size, Self::Error> {
        let (columns, rows) = self.columns_rows();
        Ok(Size {
            width: columns,
            height: rows,
        })
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        Ok(WindowSize {
            columns_rows: self.size()?,
            pixels: Size::ZERO,
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
