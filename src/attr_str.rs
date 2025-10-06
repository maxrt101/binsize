//! # `binsize::attr_str`
//!
//! Has a simple implementation of `AttributeString` - string with attributes.
//! The attributes being an abstracted ANSI color/text manipulation sequences
//!


use std::fmt::{Display, Debug, Formatter};

/// Enum for abstracting ANSI color/text manipulation sequences
/// 
/// It's not even half complete, and this crate uses maybe 6-10 sequences, but I plan on allowing
/// users to redefine color scheme sometimes in the future, so it get `allow(dead_code)` for now
/// 
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Attribute {
    TextReset,

    TextBold,
    TextDim,
    TextItalic,
    TextUnderline,
    TextBlink,
    TextInverse,
    TextHidden,
    TextStrikethrough,

    ColorFgBlack,
    ColorFgRed,
    ColorFgGreen,
    ColorFgYellow,
    ColorFgBlue,
    ColorFgMagenta,
    ColorFgCyan,
    ColorFgWhite,
    ColorFgDefault,
}

impl Display for Attribute {
    /// Converts to actual escape sequence upon formatting
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Attribute::TextReset         => f.write_str("\x1b[0m"),
            Attribute::TextBold          => f.write_str("\x1b[1m"),
            Attribute::TextDim           => f.write_str("\x1b[2m"),
            Attribute::TextItalic        => f.write_str("\x1b[3m"),
            Attribute::TextUnderline     => f.write_str("\x1b[4m"),
            Attribute::TextBlink         => f.write_str("\x1b[5m"),
            Attribute::TextInverse       => f.write_str("\x1b[7m"),
            Attribute::TextHidden        => f.write_str("\x1b[8m"),
            Attribute::TextStrikethrough => f.write_str("\x1b[9m"),
            Attribute::ColorFgBlack      => f.write_str("\x1b[30m"),
            Attribute::ColorFgRed        => f.write_str("\x1b[31m"),
            Attribute::ColorFgGreen      => f.write_str("\x1b[32m"),
            Attribute::ColorFgYellow     => f.write_str("\x1b[33m"),
            Attribute::ColorFgBlue       => f.write_str("\x1b[34m"),
            Attribute::ColorFgMagenta    => f.write_str("\x1b[35m"),
            Attribute::ColorFgCyan       => f.write_str("\x1b[36m"),
            Attribute::ColorFgWhite      => f.write_str("\x1b[37m"),
            Attribute::ColorFgDefault    => f.write_str("\x1b[39m"),
        }
    }
}

/// String with attributes
#[derive(Clone)]
pub struct AttributeString {
    /// Actual string value
    str: String,
    
    /// List of attributes
    attrs: Vec<Attribute>,
}

impl AttributeString {
    /// Creates new `AttributeString`
    pub fn new(str: &str, attrs: &[Attribute]) -> AttributeString {
        AttributeString {
            str: str.to_string(),
            attrs: attrs.to_vec(),
        }
    }

    /// Creates `AttributeString` without attributes
    pub fn from(str: &str) -> AttributeString {
        AttributeString {
            str: str.to_string(),
            attrs: Vec::new(),
        }
    }

    /// Returns underlying string's value
    pub fn len(&self) -> usize {
        self.str.len()
    }

    /// Pushes new attribute into attribute list
    pub fn push_attr(&mut self, attr: Attribute) {
        self.attrs.push(attr);
    }

    /// Returns underlying string
    pub fn string(&self) -> &String {
        &self.str
    }

    /// Applies all attributes
    pub fn attrs_apply(&self) {
        for attr in &self.attrs {
            print!("{}", attr);
        }
    }

    /// Resets all attributes
    pub fn attrs_reset(&self) {
        print!("{}", Attribute::TextReset)
    }
}


/// Creates an attributeless `AttributeString` from `&str`
impl From<&str> for AttributeString {
    fn from(str: &str) -> AttributeString {
        AttributeString::from(str)
    }
}

/// Creates an attributeless `AttributeString` from `String`
impl From<String> for AttributeString {
    fn from(str: String) -> AttributeString {
        AttributeString::from(str.as_str())
    }
}

/// Create a `AttributeString` from `&str` and `Attribute` list tuple
impl<const N: usize> From<(&str, &[Attribute; N])> for AttributeString {
    fn from(str: (&str, &[Attribute; N])) -> AttributeString {
        AttributeString::new(str.0, str.1)
    }
}

/// Create a `AttributeString` from `String` and `Attribute` list tuple
impl<const N: usize> From<(String, &[Attribute; N])> for AttributeString {
    fn from(str: (String, &[Attribute; N])) -> AttributeString {
        AttributeString::new(str.0.as_str(), str.1)
    }
}

/// Print string along with attributes
impl Display for AttributeString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for attr in &self.attrs {
            write!(f, "{}", attr)?;
        }

        write!(f, "{}", self.str)?;
        write!(f, "{}", Attribute::TextReset)
    }
}

/// Used for debug
impl Debug for AttributeString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "(\"{}\" {:?})", self.str, self.attrs)
    }
}
