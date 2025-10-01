//! # `binsize::exe`
//!
//! Executable file parsing. Uses `object` for actual parsing and retrieves a generalized
//! `ExecutableInfo` struct with symbols/sections/regions/etc. for displaying later on
//!

use object::{File, Object, ObjectSection, ObjectSegment, ObjectSymbol};
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;
use crate::util::SortOrder;

/// Symbol kind
#[derive(PartialEq)]
pub enum SymbolKind {
    Unknown,
    Function,
    Data,
}

impl Display for SymbolKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Unknown  => write!(f, "UNK "),
            SymbolKind::Function => write!(f, "FUNC"),
            SymbolKind::Data     => write!(f, "DATA"),
        }
    }
}

/// Represents a symbol (function/variable)
pub struct Symbol {
    /// Symbol name (demangled)
    pub name: String,

    /// Crate name (guessed)
    pub crate_name: String,

    /// Symbol size
    pub size: usize,

    /// Symbol address
    pub addr: usize,

    /// Symbol kind
    pub kind: SymbolKind,

    // TODO: Maybe add definition location (requires dwarf parsing most likely)
}

/// Represents a section in an executable (`.text`/`.data`/etc.)
pub struct Section {
    /// Section name
    pub name: String,

    /// Section address
    pub addr: usize,

    /// Section size
    pub size: usize,
}

/// Represents a Program Header (Segment)
pub struct Segment {
    /// Address of segment
    pub addr: usize,

    /// Size of loaded data
    pub size: usize,
}

/// Represents executable information
pub struct ExecutableInfo {
    pub symbols: Vec<Symbol>,
    pub sections: Vec<Section>,
    pub segments: Vec<Segment>,
}

impl ExecutableInfo {
    /// Sorts symbols by size, given a `SortOrder`
    pub fn sort_symbols(&mut self, order: SortOrder) {
        self.symbols.sort_by(|s1, s2|
            if match order {
                SortOrder::Ascending => s1.size < s2.size,
                SortOrder::Descending => s1.size > s2.size
            } {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            }
        );
    }
}

impl Default for ExecutableInfo {
    fn default() -> Self {
        Self {
            symbols: Vec::new(),
            sections: Vec::new(),
            segments: Vec::new(),
        }
    }
}

/// Demangles a symbol using `rustc_demangle` + removes trailing hash, that `rustc` adds
fn demangle(s: &str) -> String {
    let mut name = rustc_demangle::demangle(s).to_string();

    // Taken as-is from binfarce
    if let Some(pos) = name.bytes().rposition(|b| b == b':') {
        name.drain((pos - 1)..);
    }

    name
}

/// Compiled regex pattern for roughly guessing crate name from symbol
static CRATE_PATTERN: OnceLock<regex::Regex> = OnceLock::new();

/// Tries to guess a crate from mangled symbol. Uses `demangle()` and regex magic
fn demangle_crate(s: &str) -> String {
    // TODO: Should be improved, as sometimes it guesses wrong
    //       For example: `core  <rtrs::log::record::DefaultRecord as core::fmt::Display>::fmt`
    //       This function returned `core`, although it's an impl for core trait, but for a type in `rtrs` crate
    let re = CRATE_PATTERN.get_or_init(|| regex::Regex::new(r"^<?&?(.+as )?(dyn )?(\w+):").unwrap());

    if let Some(c) = re.captures(demangle(s).as_str()) {
        c.get(3).unwrap().as_str().to_string()
    } else {
        "?".to_string()
    }
}

/// Parses an executable
pub fn parse(path: &std::path::Path) -> Result<ExecutableInfo, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(&path)?;
    let data = unsafe { memmap2::Mmap::map(&file)? };

    let exe = File::parse(&*data)?;

    let segments = exe.segments()
        .map(
            |s| Segment {
                size: s.size() as usize,
                addr: s.address() as usize,
            }
        )
        .collect();

    let sections = exe.sections()
        .map(
            |s| Section {
                // TODO: Should add section type (`PROGBITS`/`NOBITS`/etc.) to filter later on
                name: s.name().unwrap_or("?").to_string(),
                addr: s.address() as usize,
                size: s.size() as usize,
            }
        )
        .collect();

    let mut symbols = exe.symbols()
        .map(
            |s| Symbol {
                name:       demangle(s.name().unwrap_or("?")),
                crate_name: demangle_crate(s.name().unwrap_or("?")),
                size:       s.size() as usize,
                addr:       s.address() as usize,
                kind: match s.kind() {
                    object::SymbolKind::Text => SymbolKind::Function,
                    object::SymbolKind::Data => SymbolKind::Data,
                    _                        => SymbolKind::Unknown,
                },
            }
        )
        .filter(|s| s.kind != SymbolKind::Unknown)
        .collect::<Vec<_>>();

    // Symbols need to be sorted in ascending order by address to calculate size
    symbols.sort_by_key(|s| s.addr);

    for i in 0..symbols.len() - 1 {
        let sym = &symbols[i];

        if sym.size == 0 {
            // Mach-O doesn't store symbol sizes, so they have to be calculated by hand
            // With symbols sorted, we can easily find next symbol to subtract current
            // symbol's address from the next (higher) one
            // This fix comes from binfarce macho.rs, I already started to bang my head
            // against the wall, so... much thanks to whoever found this
            // TODO: Check if sizes are valid, especially for DATA symbols
            if let Some(next) = symbols[i..].iter().skip_while(|s| s.addr == sym.addr).next() {
                // Avoid overflow: better to not have a size, than to have an invalid one
                if next.addr > sym.addr {
                    // Subtract current symbol address from next one
                    symbols[i].size = next.addr - sym.addr;
                }
            }
        }
    }

    Ok(ExecutableInfo { segments, sections, symbols })
}

