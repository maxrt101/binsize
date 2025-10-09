//! # `binsize::output`
//!
//! Implements control mechanism over output tables and their columns
//!

use std::collections::HashMap;

/// Macro to update `field_mask` (bitmask of allowed fields) by using some type that
/// implements `try_from` and returns a value that can be converted to `u8`
///
/// # Arguments
///
/// * `field_mask` - Result variable, parsed bitfield will be ORed into here
/// * `field` - String(?) value to parse from
/// * `enum` - Name of type that will perform parsing using `try_from`
///
/// # Example
///
/// ```
/// let field = "symbol=name,size";
/// let mut field_mask = 0;
/// update_field_mask_from!(field_mask, field, SymbolTableFields),
/// ```
///
macro_rules! update_field_mask_from {
    ($field_mask:expr, $field:ident, $enum:ident) => {
        $field_mask |= $enum::try_from($field)
            .expect(
                format!("Invalid value for {}: '{}'", stringify!($enum), $field).as_str()
            ) as u8
    };
}

/// Bit fields of symbol table columns/fields
pub enum SymbolTableFields {
    Size    = 1 << 0,
    Percent = 1 << 1,
    Kind    = 1 << 2,
    Crate   = 1 << 3,
    Name    = 1 << 4,
    All     = 0xFF,
}

impl TryFrom<&str> for SymbolTableFields {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use SymbolTableFields::*;

        match value {
            "*" | "all"           => Ok(All),
            "s" | "size"          => Ok(Size),
            "p" | "percent" | "%" => Ok(Percent),
            "k" | "kind"          => Ok(Kind),
            "c" | "crate"         => Ok(Crate),
            "n" | "name"          => Ok(Name),
            _                     => Err(format!("Unknown symbol table output field: '{}'", value)),
        }
    }
}

/// Bit fields of crate table columns/fields
pub enum CrateTableFields {
    Name = 1 << 0,
    Size = 1 << 1,
    All  = 0xFF,
}

impl TryFrom<&str> for CrateTableFields {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use CrateTableFields::*;

        match value {
            "*" | "all"  => Ok(All),
            "n" | "name" => Ok(Name),
            "s" | "size" => Ok(Size),
            _            => Err(format!("Unknown crate table output field: '{}'", value)),
        }
    }
}

/// Bit fields of section table columns/fields
pub enum SectionTableFields {
    Name = 1 << 0,
    Addr = 1 << 1,
    Size = 1 << 2,
    All  = 0xFF,
}

impl TryFrom<&str> for SectionTableFields {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use SectionTableFields::*;

        match value {
            "*" | "all"  => Ok(All),
            "n" | "name" => Ok(Name),
            "a" | "addr" => Ok(Addr),
            "s" | "size" => Ok(Size),
            _            => Err(format!("Unknown section table output field: '{}'", value)),
        }
    }
}

/// Bit fields of segment table columns/fields
pub enum SegmentTableFields {
    Name    = 1 << 0,
    Addr    = 1 << 1,
    Used    = 1 << 2,
    Size    = 1 << 3,
    Percent = 1 << 4,
    All     = 0xFF,
}

impl TryFrom<&str> for SegmentTableFields {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use SegmentTableFields::*;

        match value {
            "*" | "all"           => Ok(All),
            "n" | "name"          => Ok(Name),
            "a" | "addr"          => Ok(Addr),
            "u" | "used"          => Ok(Used),
            "s" | "size"          => Ok(Size),
            "p" | "percent" | "%" => Ok(Percent),
            _                     => Err(format!("Unknown segment table output field: '{}'", value)),
        }
    }
}


/// Bitmask of possible output tables
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub enum OutputKind {
    Symbols  = 1 << 0,
    Sections = 1 << 1,
    Segments = 1 << 2,
    Crates   = 1 << 3,
    None     = 0,
    All      = 0xff,
}

impl OutputKind {
    /// Returns all valid `OutputKind` values (all without `None` & `All`,
    /// which are for internal use)
    fn all() -> Vec<OutputKind> {
        vec![OutputKind::Symbols, OutputKind::Sections, OutputKind::Segments, OutputKind::Crates]
    }
}

impl TryFrom<&str> for OutputKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use OutputKind::*;

        match value {
            "*"   | "all"      => Ok(All),
            "sym" | "symbols"  => Ok(Symbols),
            "sec" | "sections" => Ok(Sections),
            "seg" | "segments" => Ok(Segments),
            "cr"  | "crates"   => Ok(Crates),
            _                  => Err(format!("Invalid output type '{}'", value)),
        }
    }
}

/// Stores allowed output tables, and their fields
pub struct Output {
    /// Bitmask of `OutputKind`
    outputs: u8,

    /// For each valid `OutputKind` store table-dependant bitmask
    ///
    /// # Key-Value Relations:
    /// * `Symbols` - `SymbolTableFields`
    /// * `Sections` - `SectionTableFields`
    /// * `Segments` - `SegmentTableFields`
    /// * `Crates` - `CrateTableFields`
    ///
    fields: HashMap<OutputKind, u8>,
}

impl Output {
    /// Create empty output
    pub fn new() -> Self {
        Default::default()
    }

    /// Enable an output for table denoted with `kind`
    pub fn enable(&mut self, kind: OutputKind) {
        self.outputs |= kind as u8;
    }

    /// Disable an output for table denoted with `kind`
    pub fn disable(&mut self, kind: OutputKind) {
        self.outputs &= !(kind as u8);
    }

    /// Returns true if table denoted by `kind` is enabled for output
    pub fn enabled(&self, kind: OutputKind) -> bool {
        self.outputs & (kind as u8) != 0
    }

    /// Returns `true` if any output is enabled
    pub fn any_enabled(&self) -> bool {
        self.outputs != 0
    }

    /// Disables a column `field` in table denoted by `kind`
    pub fn field_disable(&mut self, kind: OutputKind, field: u8) {
        if let Some(value) = self.fields.get_mut(&kind) {
            *value &= !field;
        }
    }

    /// Returns true if column `field` in table denoted by `kind` is enabled for output
    pub fn field_enabled(&self, kind: OutputKind, field: u8) -> bool {
        if let Some(value) = self.fields.get(&kind) {
            value & field != 0
        } else {
            false
        }
    }

    /// Parse & apply an output pattern
    ///
    /// # Example
    ///
    /// ```
    /// let mut output = Output::default();
    /// output.apply_pattern("sections=name,size");
    /// output.apply_pattern("segments=name,used,size");
    /// ```
    ///
    pub fn apply_pattern(&mut self, pattern: &str) {
        let mut enable = true;
        let output_kind: OutputKind;
        let mut field_mask = 0;

        // If pattern start with `!` - it's a disable/disallow pattern, so invert `enable` and skip
        // first symbol (`!`)
        let pattern = if pattern.starts_with('!') {
            enable = false;
            pattern.strip_prefix('!').unwrap()
        } else {
            pattern
        };

        // If pattern contains `=` - field/column list is specified
        if pattern.contains('=') {
            let (kind, fields) = pattern.split_once('=').unwrap();

            output_kind = OutputKind::try_from(kind)
                .expect(format!("Unknown output kind: '{}'", kind).as_str());

            // By parsing `OutputKind` first, we now know which `*TableFields` to use for
            // column/fields parsing
            for field in fields.split(',') {
                match output_kind {
                    OutputKind::Symbols  => update_field_mask_from!(field_mask, field, SymbolTableFields),
                    OutputKind::Sections => update_field_mask_from!(field_mask, field, SectionTableFields),
                    OutputKind::Segments => update_field_mask_from!(field_mask, field, SegmentTableFields),
                    OutputKind::Crates   => update_field_mask_from!(field_mask, field, CrateTableFields),
                    _                    => panic!("Can't specify output fields for '{}'", kind)
            }
            }
        } else {
            output_kind = OutputKind::try_from(pattern)
                .expect(format!("Invalid output kind: '{}'", pattern).as_str());

            // No column list, so enable all
            field_mask = 0xFF;
        }

        if enable {
            self.enable(output_kind);
        } else {
            self.disable(output_kind);
        }

        if let Some(mask) = self.fields.get_mut(&output_kind) {
            if enable {
                *mask = field_mask;
            } else {
                *mask = !field_mask;
            }
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        let mut out = Self {
            // By default, disallow all output
            outputs: OutputKind::None as u8,
            fields:  HashMap::new(),
        };

        // By default, allow all columns to be printed
        for kind in OutputKind::all() {
            out.fields.insert(kind, 0xFF);
        }

        out
    }
}
