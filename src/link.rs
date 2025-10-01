//! # `binsize::link`
//!
//! Houses a linker memory region parser
//!

use std::error::Error;
use std::fmt::Display;
use std::sync::OnceLock;
use crate::exe::LoadSegment;

/// Compiled regex pattern for matching memory region declaration under MEMORY in LD scripts
static MEM_REG_PATTERN: OnceLock<regex::Regex> = OnceLock::new();

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

    /// Parse memory region declarations from linker script
    pub fn from_file(path: &std::path::PathBuf) -> Result<Vec<Self>, Box<dyn Error>> {
        let s = std::fs::read_to_string(path)?;

        // TODO: Check if anything other than declarations from MEMORY can be matched here (by passing whole linker script for example)
        let re = MEM_REG_PATTERN.get_or_init(|| regex::Regex::new(r"^\s+(\w+)\s+:\s+(\w+)\s+=\s+(\w+),\s+(\w+)\s+=\s+(\w+)").unwrap());

        let mut regions = Vec::new();

        for line in s.split("\n") {
            if let Some(cap) = re.captures(line) {
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
                            // TODO: This will fail if some psycho decides to declare ORIGIN in decimal
                            origin = usize::from_str_radix(
                                cap.get(i+1)
                                    .ok_or_else(|| Self::create_error("Expected base 16 integer after ORIGIN"))?
                                    .as_str()
                                    .strip_prefix("0x")
                                    .ok_or_else(|| Self::create_error("Expected base 16 integer after ORIGIN, prefixed with 0x"))?,
                                16
                            )?;
                        }
                        "LENGTH" => {
                            // Parse actual value, which allways comes in the next capture group
                            // TODO: Will fail if size is not expressed as "<base 10 int>K", e.g. as base 16 or through a variable
                            length = usize::from_str_radix(
                                cap.get(i+1)
                                    .ok_or_else(|| Self::create_error("Expected base 10 integer after LENGTH"))?
                                    .as_str()
                                    .strip_suffix("K")
                                    .ok_or_else(|| Self::create_error("Expected base 10 integer after ORIGIN, suffixed with K"))?,
                                10
                            )? * 1024;
                        }
                        _ => {
                            return Err(Self::create_error(format!("Expected ORIGIN or LENGTH, got {}", val).as_str()));
                        }
                    }
                }

                regions.push(MemoryRegion::new(name.as_str(), origin, length))
            }
        }

        Ok(regions)
    }

    /// Uses program headers (LOAD segments) from parsed binary to enrich regions, parsed from
    /// linker script, with actual usage data
    pub fn use_segments_data(regions: &mut Vec<MemoryRegion>, segments: &Vec<LoadSegment>) {
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