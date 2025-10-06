//! # `binsize::table`
//!
//! Implements simple `Table` for pretty-printing data
//!

use std::fmt::{Debug, Formatter};
use std::ops::{Index, IndexMut};

use crate::attr_str::{AttributeString};
use crate::util;

/// Represents left/right padding
#[derive(Clone, Copy)]
pub enum Padding {
    None,
    Left,
    Right,
}

impl Debug for Padding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Padding::None  => f.write_str("?"),
            Padding::Left  => f.write_str("L"),
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

impl Debug for Row {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.values)
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

    /// Max width of single column value. If 0 - will be initialized from `util::term_width()`
    max_width: usize,
}

impl Table {
    /// Creates new table
    pub fn new(header: Row, padding: &[Padding], rows: &[Row], max_width: usize) -> Self {
        let mut table = Self {
            header,
            padding: padding.to_vec(),
            rows: vec![],
            widths: vec![],
            max_width: if max_width == 0 { util::term_width() } else { max_width }
        };

        // Total size of header row in symbols
        let mut size = 0;

        for val in table.header.values.iter() {
            // Current column size
            let mut col_size = val.len();

            // If size of already processed columns and size of current column exceeds `max_width`
            if size + col_size > table.max_width {
                // Trim `col_size` to space, that's left (`max_width` - `size`)
                col_size = table.max_width - size;
            }

            // Push `col_size` to cached widths
            table.widths.push(col_size);

            // Update header row size
            size += col_size;
        }

        for row in rows {
            // Manually add each row, for widths cache to be updated
            table.push_row(row.clone()).unwrap()
        }

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

        // Total size of row in symbols
        let mut size: usize = 0;

        for (i, value) in row.values.iter().enumerate() {
            // Size of column
            let mut col_size = value.len();

            // If size of already processed columns and size of current column exceeds `max_width`
            if size + col_size > self.max_width {
                // Trim `col_size` to space, that's left (`max_width` - `size`)
                col_size = self.max_width - size - 1;
            }

            // If `col_size` is bigger than cached max width for current column
            if col_size > self.widths[i] {
                // Update cached value
                self.widths[i] = col_size;
            } else {
                // Check if cached value doesn't already exceed `max_width`
                if size + self.widths[i] > self.max_width {
                    // Reduce cached value to actual max value for this column
                    self.widths[i] = col_size;
                }
                // Set `col_size` to relevant value from cache
                col_size = self.widths[i];
            }

            // Update row size
            size += col_size + 1;
        }

        // Save row
        self.rows.push(row);

        Ok(())
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
        // Total size of row in symbols
        let mut size = 0;

        for (i, val) in row.iter().enumerate() {
            if ignore_empty && val.len() == 0 {
                return;
            }

            // Creates `str` - column value, trimmed to `max_width`, if needed, and `overflowed` -
            // leftover/trimmed part of column, which can't fit in original row
            let (str, overflowed) =  if size + val.len() > self.max_width {
                // If current column can't fit - split it into 2 parts - first is printed in
                // current column (and fits into `max_width` along with everything that was already
                // printed), and second - which is padded, and printed in the next row
                let (part1, part2) = val.string().split_at(self.max_width - size - 1);
                (part1, Some(part2))
            } else {
                // If current column fits - return it as-is
                (val.string().as_str(), None)
            };

            // Applies any text/color modifications
            val.attrs_apply();

            match if i >= self.padding.len() {
                Padding::None
            } else {
                self.padding[i]
            } {
                Padding::None => {
                    print!("{}", str);
                }
                Padding::Left => {
                    print!("{:width$}", str, width = self.widths[i]);
                }
                Padding::Right => {
                    print!("{:>width$}", str, width = self.widths[i]);
                }
            }

            if let Some(overflowed) = overflowed {
                // If overflowed text is present - remove attributes (so that, for example BG
                // color isn't printed to the end on the line)
                val.attrs_reset();
                println!();
                // Reapply attributes
                val.attrs_apply();
                // Print overflowed text in the next line, left-padded with spaces to the start
                // of original column
                print!("{:width$}{}", "", overflowed, width = size);
            }

            // Resets all text modifications
            val.attrs_reset();

            // Update size with max width of current column
            size += self.widths[i];
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
