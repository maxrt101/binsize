//! # `binsize::link`
//!
//! Houses a linker memory region parser
//!

use std::error::Error;
use std::fmt::Display;
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::exe::Segment;

/// Compiled regex pattern for matching memory region declaration under MEMORY in LD scripts
static MEM_REG_PATTERN: OnceLock<regex::Regex> = OnceLock::new();

/// Compiled regex pattern for matching variable declarations in LD scripts
static VARIABLE_PATTERN: OnceLock<regex::Regex> = OnceLock::new();

/// Represents a memory region, defined in LD script. Also stores some properties, which
/// are calculated later using program headers from parsed binary
///
/// Example:
/// ```
/// let mut exe = exe::parse(exe_path).unwrap();
///
/// let mut regions = link::MemoryRegion::from_file(ld_path).unwrap();
///
/// link::MemoryRegion::use_segments_data(&mut regions, &exe.segments);
/// ```
///
pub struct MemoryRegion {
    /// Region name
    pub name: String,

    /// Base (start address) of a region
    pub origin: usize,

    /// Size of a region
    pub length: usize,

    /// How much is used
    pub used: usize,

    /// How much is used in percentage to `length`
    pub used_percentage: f32,
}

impl MemoryRegion {
    /// Create a memory region from data, parsed from linker script
    pub fn new(name: &str, origin: usize, length: usize) -> Self {
        Self {
            name: name.to_string(), origin, length, used: 0, used_percentage: 0.0
        }
    }

    /// Lower and upper bound (addressed) of a region
    pub fn bounds(&self) -> (usize, usize) {
        (self.origin, self.origin + self.length)
    }

    /// Helper function to create a generic boxed error from a message
    fn create_error(str: &str) -> Box<dyn Error> {
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, str))
    }

    /// Helper function that parses a value, Possible values:
    ///  - base 10 integer
    ///  - base 16 integer (prefixed with `0x`)
    ///  - base 10 integer suffixed with `K` (kilobytes)
    ///  - base 10 integer suffixed with `M` (megabytes)
    ///  - Variable reference (to previously parsed variable with `NAME = VALUE;` syntax)
    fn parse_value(vars: &HashMap<String, usize>, val: &str) -> Result<usize, Box<dyn Error>> {
        if val.starts_with("0x") {
            return Ok(usize::from_str_radix(val.strip_prefix("0x").unwrap(), 16)?);
        }

        if val.ends_with("K") {
            return Ok(usize::from_str_radix(val.strip_suffix("K").unwrap(), 10)? * 1024);
        }

        if val.ends_with("M") {
            return Ok(usize::from_str_radix(val.strip_suffix("M").unwrap(), 10)? * 1024 * 1024);
        }

        if let Ok(x) = val.parse() {
            return Ok(x);
        }

        if vars.contains_key(val) {
            return Ok(*vars.get(val).unwrap());
        }

        Err(Self::create_error(format!("Can't find value for variable '{}'", val).as_str()))
    }

    /// Helper function for parsing variable value, and inserting it into the variable map
    fn parse_var(vars: &mut HashMap<String, usize>, name: &str, val: &str) -> Result<(), Box<dyn Error>> {
        let val = Self::parse_value(vars, val)?;

        vars.insert(name.to_string(), val);

        Ok(())
    }

    /// Helper function for parsing memory region declaration
    fn parse_region(cap: &regex::Captures<'_>, vars: &HashMap<String, usize>) -> Result<MemoryRegion, Box<dyn Error>> {
        // First group captures memory region name
        let name = cap.get(1)
            .ok_or_else(|| Self::create_error("Expected memory region name"))?
            .as_str()
            .to_string();

        let mut origin = 0usize;
        let mut length = 0usize;

        // I don't really know if ORIGIN's & LENGTH's order can be swapped, but to be sure
        // this iterates over possible capture group positions of both ORIGIN & LENGTH,
        // and parses whichever is in that particular group
        for i in [2, 4] {
            let val = cap
                .get(i)
                .ok_or_else(|| Self::create_error("Expected ORIGIN or LENGTH"))?
                .as_str();

            match val {
                "ORIGIN" => {
                    // Parse actual value, which allways comes in the next capture group
                    let val = cap.get(i+1)
                        .ok_or_else(|| Self::create_error("Expected a value after ORIGIN"))?
                        .as_str();

                    origin = Self::parse_value(&vars, val)?;
                }
                "LENGTH" => {
                    // Parse actual value, which allways comes in the next capture group
                    let val = cap.get(i+1)
                        .ok_or_else(|| Self::create_error("Expected a value after LENGTH"))?
                        .as_str();

                    length = Self::parse_value(&vars, val)?;
                }
                _ => {
                    return Err(Self::create_error(format!("Expected ORIGIN or LENGTH, got {}", val).as_str()));
                }
            }
        }

        Ok(MemoryRegion::new(name.as_str(), origin, length))
    }

    /// Parse memory region declarations from linker script
    ///
    /// Will parse variable declarations and memory regions, will work on something like this:
    ///
    /// ```rust,ignore
    /// __boot_size = 0x10000; /* 64K */
    /// __slot_size = 0x16800; /* 90K */
    ///
    /// MEMORY
    /// {
    ///       BOOTLOADER  : ORIGIN = 0x8000000,  LENGTH = __boot_size
    ///       APPLICATION : ORIGIN = 0x8010000,  LENGTH = __slot_size
    ///       BACKUP      : ORIGIN = 0x8020000,  LENGTH = __slot_size
    ///       RAM         : ORIGIN = 0x20000000, LENGTH = 32K
    /// }
    /// ```
    ///
    /// However, will not work with anything other, e.g.: simple expressions (`8K + 10K`),
    /// references to other segments (`ORIGIN(RAM) + LENGTH(RAM)`), this is a known limitation
    /// right now. For complex expressions to work, better parser needs to be built (one
    /// that doesn't rely on regexps for parsing)
    ///
    pub fn from_file(path: &std::path::PathBuf) -> Result<Vec<Self>, Box<dyn Error>> {
        let s = std::fs::read_to_string(path)?;

        // TODO: Check if anything other than declarations from MEMORY can be matched here (by passing whole linker script for example)
        let mem_reg_re = MEM_REG_PATTERN.get_or_init(||
            regex::Regex::new(r"^\s*(\w+)\s*:\s*(\w+)\s*=\s*(\w+),\s*(\w+)\s*=\s*(\w+)").unwrap()
        );

        let var_re = VARIABLE_PATTERN.get_or_init(||
            regex::Regex::new(r"^\s*(\w+)\s*?=\s*(\w+)\s*;").unwrap()
        );

        let mut vars = HashMap::new();

        let mut regions = Vec::new();

        for line in s.split("\n") {
            if let Some(cap) = var_re.captures(line) {
                Self::parse_var(
                    &mut vars,
                    cap.get(1)
                        .expect("Expected variable name")
                        .as_str(),
                    cap.get(2)
                        .expect("Expected variable value")
                        .as_str()
                )?;
            }

            if let Some(cap) = mem_reg_re.captures(line) {
                regions.push(Self::parse_region(&cap, &vars)?)
            }
        }

        Ok(regions)
    }

    /// Uses program headers (LOAD segments) from parsed binary to enrich regions, parsed from
    /// linker script, with actual usage data
    pub fn use_segments_data(regions: &mut Vec<MemoryRegion>, segments: &Vec<Segment>) {
        for reg in regions.iter_mut() {
            let (start, end) = reg.bounds();

            for seg in segments.iter() {
                if start <= seg.addr && seg.addr <= end {
                    reg.used += seg.size;
                }
            }

            reg.used_percentage = reg.used as f32 / (reg.length as f32 / 100.0)
        }
    }
}

impl Display for MemoryRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Just mimics format from linker script
        write!(f, "{} : ORIGIN = 0x{:x}, LENGTH = {}K", self.name, self.origin, self.length / 1024)
    }
}