//!
//!
//!

use std::sync::OnceLock;

/// Kind of demangled symbol by language
#[derive(PartialEq, Eq)]
pub enum DemangledSymbolKind {
    Rust,
    Cpp,
    Other
}

/// Demangled symbol
pub struct DemangledSymbol {
    pub kind: DemangledSymbolKind,
    pub name: String,
}

/// Demangles a symbol using `rustc_demangle` + removes trailing hash, that `rustc` adds
/// If demangling wasn't successful, will try to treat it as a C++ symbol, and if that also
/// fails - will return mangled version
pub fn demangle(s: &str) -> DemangledSymbol {
    let mut name = rustc_demangle::demangle(s).to_string();

    // If demangling as rust symbol was successful
    if name != s {
        // Remove hash, that rustc adds at the end of every symbol
        // Taken as-is from binfarce
        if let Some(pos) = name.bytes().rposition(|b| b == b':') {
            name.drain((pos - 1)..);
        }

        return DemangledSymbol {
            kind: DemangledSymbolKind::Rust,
            name
        };
    } else {
        // Try with C++ demangler
        if let Ok(sym) = cpp_demangle::Symbol::new(s) {
            if let Ok(val) = sym.demangle() {
                return DemangledSymbol {
                    kind: DemangledSymbolKind::Cpp,
                    name: val
                };
            }
        }
    }

    // Return symbol name as-is
    DemangledSymbol {
        kind: DemangledSymbolKind::Other,
        name: s.to_string(),
    }
}

/// Compiled regex pattern for roughly guessing crate name from symbol
static CRATE_PATTERN: OnceLock<regex::Regex> = OnceLock::new();

/// Tries to guess a crate from mangled symbol. Uses regex magic
pub fn crate_name_from_demangled(s: &str) -> String {
    // TODO: Rewrite
    //
    // This *should* match most symbols
    //
    // It works by matching (and discarding) any of `<`, `&`, `*`, `const`, `mut` `dyn` and then
    // matching either `\w+:` (which is an immediate crate name, like `rtrs` in
    // `rtrs::task::Task<R>::new`, or matching `as \w+:` (which is crate name for trait, method's
    // of which are being implemented, like `core` in `<T as core::any::Any>::type_id`), if first
    // match was unsuccessful.
    //
    // Most of the time, first match (`rtrs` in `rtrs::task::Task<R>::new`) is sufficient, but
    // with trait impls it's more complex.
    //
    // My reasoning is that the crate for an impl should be
    // the crate of type, which implements a trait, not crate of the trait.
    //
    // But sometimes an integral type (or `T`) implements some trait, if that happens, this code
    // will consider trait's crate to be the correct one.
    //
    // As for generics instantiation for concrete types: `core::ptr::drop_in_place<rtrs::RwLock>`,
    // I think `core` should be matched as the crate, because `drop_in_place` is defined in `core`,
    // even if instantiating type is from another crate, the code of `drop_in_place` is still in
    // `core`
    //
    // # Examples
    //
    // With simple symbols, such as `core::fmt::Formatter::write_str` - `core` (first token in `::`
    // chain) will be matched as crate name.
    //
    // For simple impls, such as `<heapless::vec::Vec<T,_> as core::ops::deref::Deref>::deref` -
    // `heapless` (first token in `::` chain of type that implements the trait) will be matched as
    // crate name.
    //
    // For impls for integral or generic types, such as `<bool as core::fmt::Display>::fmt` or
    // `<*mut T as core::fmt::Debug>::fmt` - `core` will get matched
    //
    let re = CRATE_PATTERN.get_or_init(||
        regex::Regex::new(r"^<?[&*]?(mut )?(const )?(dyn )?((\w+):)?(.*as (\w+):)?").unwrap()
    );

    if let Some(c) = re.captures(s) {
        let crate_name1 = if let Some(name) = c.get(5) {
            name.as_str()
        } else {
            ""
        };

        let crate_name2 = if let Some(name) = c.get(7) {
            name.as_str()
        } else {
            ""
        };

        if !crate_name1.is_empty() {
            return crate_name1.to_string();
        }

        if !crate_name2.is_empty() {
            return crate_name2.to_string();
        }
    }

    "?".to_string()
}