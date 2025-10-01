//! # `binsize::table`
//!
//! Implements simple `Table` for pretty-printing data
//!

use std::fmt::{Debug, Formatter};
use std::ops::{Index, IndexMut};

use crate::attr_str::{AttributeString};

/// Represents left/right padding
#[derive(Clone, Copy)]
pub enum Padding {
    Left,
    Right,
}

impl Debug for Padding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Padding::Left => f.write_str("L"),
            Padding::Right => f.write_str("R"),
        }
    }
}

/// Represents Row of data in a table
#[derive(Clone)]
pub struct Row {
    /// Pack of values (column data)
    /// `AttributeString` is used here to enable multicolored rows to be printed
    values: Vec<AttributeString>
}

impl Row {
    /// Creates new Row from AttributeString slice
    pub fn new(values: &[AttributeString]) -> Self {
        Self { values: Vec::from(values) }
    }

    /// Returns length of value pack
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Push new value into the row
    pub fn push(&mut self, value: AttributeString) {
        self.values.push(value);
    }

    /// Map each `AttributeString` inside a row, into another `AttributeString`
    /// while consuming original `Row` and returning a new one
    ///
    /// TODO: It seems pretty strange that while `Row` is consumed, `F` still
    ///       must receive a reference instead of moved value?
    pub fn map<F: FnMut(&AttributeString) -> AttributeString>(self, f: F) -> Self {
        Row::new(self.values.iter().map(f).collect::<Vec<_>>().as_slice())
    }
}

impl Default for Row {
    fn default() -> Self {
        Self {
            values: Vec::new()
        }
    }
}

impl Index<usize> for Row {
    type Output = AttributeString;

    fn index(&self, index: usize) -> &Self::Output {
        self.values.index(index)
    }
}

impl IndexMut<usize> for Row {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.values.index_mut(index)
    }
}


/// Represents a table of rows
///
/// Example:
/// ```
///
/// let mut table = Table::with_header_and_padding(
///     Row::from(["Name ", "Address ", "Used ", "Size ", "Percentage "]),
///     &[Padding::Left, Padding::Left, Padding::Right, Padding::Right, Padding::Right],
/// );
///
/// table.push_row(
///     [
///         reg.name,
///         format!("0x{:08x} ", reg.origin),
///         format!("{} ",       reg.used),
///         format!("{} ",       reg.length),
///         format!("{} ",       reg.used_percentage),
///     ].into()
/// ).unwrap();
///
/// table.print();
///
/// ```
pub struct Table {
    /// Table Header
    header: Row,

    /// Padding for each column
    padding: Vec<Padding>,

    /// Row data
    rows: Vec<Row>,

    /// Maximal width of each column, updated on push
    widths: Vec<usize>,

    /// Max width of single column value
    max_width: usize,
}

impl Table {
    /// Creates new table
    /// Will implicitly calculate `widths`
    pub fn new(header: Row, padding: &[Padding], rows: &[Row], max_width: usize) -> Self {
        let mut table = Self {
            header,
            padding: padding.to_vec(),
            rows: rows.to_vec(),
            widths: Vec::new(),
            max_width,
        };

        table.widths = table.calculate_max_widths();

        table
    }

    /// Creates new table, given only the header
    pub fn with_header(header: Row) -> Self {
        Self::new(header, &[], &[], 0)
    }

    /// Creates new table, given only the header and padding for each column
    pub fn with_header_and_padding(header: Row, padding: &[Padding]) -> Self {
        Self::new(header, padding, &[], 0)
    }

    /// Creates new table with empty header, from number of columns
    pub fn with_empty_header(values: usize) -> Self {
        let mut header = Row::default();

        for _ in 0..values {
            header.push("".into());
        }

        Self::with_header(header)
    }

    /// Creates new table with empty header, from number of columns and padding for each column
    pub fn with_empty_header_and_padding(padding: Vec<Padding>) -> Self {
        let mut table = Self::with_empty_header(padding.len());

        table.padding = padding;

        table
    }

    /// Trims given `AttributeString` to `Self::max_width - 3` and appending `...` at the end
    /// Trims only if length of `AttributeString` exceeds `Self::max_width - 3`
    fn trim(&self, mut s: AttributeString) -> AttributeString {
        if self.max_width != 0 && s.len() >= self.max_width - 3 {
            s.truncate(self.max_width - 3);
            s.push_str("...");
        }
        s
    }

    /// Setter for `max_width`
    pub fn set_max_width(&mut self, max_width: usize) {
        self.max_width = max_width;
    }

    /// Checks that row has same number of elements as the header
    fn check_row(&self, data: &[AttributeString]) -> Result<(), String> {
        if !self.header.values.is_empty() && data.len() != self.header.len() {
           Err(format!("Row '{:?}' can't be added because of mismatching length", data))
        } else {
            Ok(())
        }
    }

    /// Push row into the table
    pub fn push_row(&mut self, row: Row) -> Result<(), String> {
        self.check_row(&row.values)?;

        self.rows.push(
            row.map(|s| self.trim(s.clone()))
        );

        for (i, value) in self.rows.last().unwrap().values.iter().enumerate() {
            if value.len() > self.widths[i] {
                self.widths[i] = value.len();
            }
        }

        Ok(())
    }

    /// Calculate max widths of each column, from header and rows
    fn calculate_max_widths(&self) -> Vec<usize> {
        let mut widths = Vec::new();

        for val in self.header.values.iter() {
            widths.push(val.len());
        }

        for row in self.rows.iter() {
            for (i, val) in row.values.iter().enumerate() {
                if val.len() > widths[i] {
                    widths[i] = val.len();
                }
            }
        }

        widths
    }

    /// Prints single row
    ///
    /// Will use
    ///  - `Self::padding` to correctly pad the value in each column and
    ///  - `AttributeString::attrs` to colorize the string
    ///
    /// `ignore_empty` - will not print, if at least one of the values is empty
    ///
    fn print_row(&self,row: &Vec<AttributeString>, ignore_empty: bool) {
        for (i, val) in row.iter().enumerate() {
            if ignore_empty && val.len() == 0 {
                return;
            }

            // Applies any text/color modifications
            val.attrs_apply();

            match if i >= self.padding.len() {
                Padding::Left
            } else {
                self.padding[i]
            } {
                Padding::Left => {
                    print!("{:width$} ", val.string(), width = self.widths[i]);
                }
                Padding::Right => {
                    print!("{:>width$} ", val.string(), width = self.widths[i]);
                }
            }

            // Resets all text modifications
            val.attrs_reset();
        }

        println!();
    }

    /// Prints whole table
    pub fn print(&self) {
        // `ignore_empty` is used to print tables without the header
        // For example in `ArgumentParser::print_help()`
        self.print_row(&self.header.values, true);

        for row in self.rows.iter() {
            self.print_row(&row.values, false);
        }
    }
}

impl Index<usize> for Table {
    type Output = Row;

    fn index(&self, index: usize) -> &Self::Output {
        self.rows.index(index)
    }
}

impl IndexMut<usize> for Table {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.rows.index_mut(index)
    }
}

/// Helper to convert an array of &str into a Row
impl<const N: usize> From<[&str; N]> for Row {
    fn from(value: [&str; N]) -> Row {
        Row::new(
            value
                .iter()
                .map(|a| <&str as Into<AttributeString>>::into(*a))
                .collect::<Vec<_>>().as_slice()
        )
    }
}

/// Helper to convert an array of String into a Row
impl<const N: usize> From<[String; N]> for Row {
    fn from(value: [String; N]) -> Row {
        Row::new(
            value
                .iter()
                .map(|a| <&str as Into<AttributeString>>::into(a.as_str()))
                .collect::<Vec<_>>().as_slice()
        )
    }
}
