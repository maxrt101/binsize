//! # `binsize`
//!
//! `binsize` is a command-line utility that provides analysis of size/memory usage of rust
//! binaries. It was inspired by `cargo-bloat`, but with a different approach to retrieving
//! symbols. Main difference is that `binsize` parses *all* symbols (both functions and
//! data/constants), except for those with a size of 0. `binsize` also provides colored output,
//! advanced output control, sections usage & memory region usage (if provided with a path to
//! linker script that has a `MEMORY` definition)
//!
//! Note: `binsize` was tested with C/C++ executables, and should work by passing them with `--file`.
//!
//! Note: file, that is being analyzed, must have `.symtab` section, otherwise `binsize` won't
//! be able to parse exported symbols. So don't strip your binaries, if you want this to work.
//!
//! Note: this is only a prototype, bugs are expected.
//!
//! ## Usage
//!
//! Navigate to you project (folder containing `Cargo.toml`) and run
//!
//! ```rust,ignore
//! $ binsize
//! ```
//!
//! You should see a symbol table with columns:
//! `Size`        - Size of symbol in bytes
//! `Percentage`  - Size of symbol as a percentage of size of all symbols combined
//! `Symbol Kind` - Symbols Type, either `FUNC` of `DATA` (if symbol kind couldn't be parsed, it'll
//!                 display `UNK`)
//! `Crate Name`  - Crate name derived from demangled symbol name. Isn't super accurate for now
//! `Symbol Name` - Demangled symbol name
//!
//! And a section table with columns:
//! `Name`    - Section name
//! `Address` - Section address
//! `Size`    - Section size
//!
//! And also a crate sizes table with columns:
//! `Crate Name` - Crate name
//! `Size`       - Size of crate (calculated from symbols)
//!
//! Note: `Crate Name` fields in symbols and crates tables are derived from demangled symbol name.
//! Currently, crate name is a rough guess, it's a known issue.
//!
//! If you want to analyze artifact, produced with a different cargo profile, use `--profile`/`-p`
//! flag:
//!
//! ```rust,ignore
//! $ binsize --profile release
//! ```
//!
//! If you want to skip building through cargo, or want to analyze some other binary, pass a path
//! to said file using `--file`:
//!
//! ```rust,ignore
//! $ binsize --file ~/projects/super-cool-project/target/release/super-cool-project
//! ```
//!
//! If you want to enable colored output, use `--color`/`-c` flag:
//!
//! ```rust,ignore
//! $ binsize --color
//! ```
//!
//! With enabled colorful output, you'll see that `Size` & `Percentage` columns became green,
//! yellow or red. This serves as a visual indicator, of a symbol being too large. Threshold for
//! symbol's size/percentage to become yellow/red can be overridden using `--size-threshold` and
//! `--percentage-threshold`:
//!
//! ```rust,ignore
//! $ binsize --percentage-threshold 1.2 5.0 --size-threshold 500 1200
//! ```
//!
//! If you want to sort symbols by size, use `--asc`/`-a` or `--desc`/`-d`:
//!
//! ```rust,ignore
//! $ binsize --asc
//! ```
//!
//! If you want to specify what information you'd like to see - use `--output`/`-o`. Possible
//! values are: `sym/symbols`, `sec/sections`, `seg/segments`, `cr/crates`, `*/all`. Columns
//! for each output table can be specified using `OUTPUT=FIELDS` syntax (where `OUTPUT` is one
//! of aforementioned values and `FIELDS` is a comma-separated list of columns).
//! For symbol table possible fields are: `*/all`, `s/size`, `%/p/percent`, `k/kind`, `c/crate`,
//! `n/name`.
//! For crate table possible fields are: `*/all`, `n/name`, `s/size`.
//! For section table possible fields are: `*/all`, `n/name`, `a/addr`, `s/size`.
//! For segment table possible fields are: `*/all`, `n/name`, `a/addr`, `u/used`, `s/size`,
//! `%/p/percent`.
//! By default, only `symbols` are shown:
//!
//! ```rust,ignore
//! $ binsize --output sections --output crates
//! ```
//!
//! It is also possible to disallow a previously allowed output by using `!`:
//!
//! ```rust,ignore
//! $ binsize --output !sections
//! ```
//!
//! If you want to filter symbols by some pattern - use `-f`/`--filter`. Filters support regex:
//!
//! ```rust,ignore
//! $ binsize --filter "core.+fmt"
//! ```
//!
//! For embedded projects, I really like GCC's --print-memory-usage linker flag, but using rust and
//! cargo, I found it pretty hard to display the information about memory region usage (FLASH/RAM).
//! So `binsize` provides a way to get that information, albeit not without user input. To get
//! memory region usage, you must pass a path to linker script, which has a `MEMORY` declaration,
//! like this:
//!
//! ```rust,ignore
//! MEMORY
//! {
//!   FLASH : ORIGIN = 0x8000000,  LENGTH = 64K
//!   RAM   : ORIGIN = 0x20000000, LENGTH = 8K
//! }
//! ```
//!
//! The `--ld-memory-map`/`-l` is used to pass the path:
//!
//! ```rust,ignore
//! $ binsize --ld-memory-map boards/stm32l051/memory.x
//! ```
//!
//! After running this, you'll get a table at the very bottom of the output with columns:
//! `Name`       - Name of memory region as defined in linker script
//! `Address`    - Base of a region. Corresponds to ORIGIN in linker script
//! `Used`       - How much of region is used, calculated using info from parsed program headers
//! `Size`       - Full size of a region. Corresponds to LENGTH in linker script
//! `Percentage` - Percentage of used against full size
//!
//! Note: If ORIGIN or LENGTH contains a complex expression (arithmetics or reference to another
//! segment), linker script parsing will fail, this is known limitation right now
//!
//! ## Config
//!
//! `binsize` also support persistent configuration stored in `.cargo/binsize.toml`
//! Here's an example of such config:
//!
//! ```rust,ignore
//! [binsize]
//! color = true
//! profile = "release"
//! output = ["symbols", "segments"]
//! file = "target/release/app"
//! ld-file = "boards/stm32l051/memory.x"
//! filter = "std"
//! sort = "asc"
//! size-threshold = [5000, 10000]
//! percentage-threshold = [0.5, 1.0]
//! ```
//!
//! Note: command line arguments will override config values
//!

use std::collections::HashMap;
use crate::util::SortOrder;
use crate::cargo::BuildOptions;
use crate::table::{Padding, Row, Table};
use crate::exe::{ExecutableInfo, SymbolKind};
use crate::attr_str::{Attribute, AttributeString};
use crate::output::{
    Output,
    OutputKind,
    SymbolTableFields,
    CrateTableFields,
    SectionTableFields,
    SegmentTableFields
};

mod cargo;
mod exe;
mod args;
mod table;
mod util;
mod attr_str;
mod link;
mod output;
mod demangle;

/// `binsize` version
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `binsize` config file location
const CONFIG: &str = ".cargo/binsize.toml";


/// Helper function for applying styling to column headers
fn color_header_fn(s: &mut AttributeString) {
    s.push_attr(Attribute::TextBold);
}

/// `binsize` Application
struct Binsize {
    /// Cargo build options
    build_options: BuildOptions,

    /// Filter for symbol names
    filter: regex::Regex,

    /// Linker script path with `MEMORY` declaration
    ld_file: String,

    /// File to parse (if `None` - will try to extract file from `cargo build`)
    file: String,

    /// Colorful output toggle
    color: bool,

    /// Sorting order of symbols
    symbols_sorting_order: Option<SortOrder>,

    /// Threshold in percent of total size for symbol to be colored yellow
    percentage_threshold_yellow: f32,

    /// Threshold in percent of total size for symbol to be colored red
    percentage_threshold_red: f32,

    /// Threshold in bytes for symbol to be colored yellow
    size_threshold_yellow: usize,

    /// Threshold in bytes for symbol to be colored red
    size_threshold_red: usize,

    /// Output control context
    output: Output,

    /// Executable info
    exe: ExecutableInfo,
}

impl Binsize {
    /// Create new `binsize` application
    fn new() -> Self {
        Self {
            build_options:               Default::default(),
            filter:                      regex::Regex::new(".+").unwrap(),
            ld_file:                     "".to_string(),
            file:                        "".to_string(),
            color:                       false,
            output:                      Output::new(),
            exe:                         Default::default(),
            symbols_sorting_order:       None,
            size_threshold_yellow:       200,
            size_threshold_red:          500,
            percentage_threshold_yellow: 0.5,
            percentage_threshold_red:    1.0,
        }
    }

    /// Parse config in `.cargo/binsize.toml`, if available
    fn parse_config(&mut self) {
        if !matches!(std::fs::exists(CONFIG), Ok(true)) {
            return;
        }

        let config = std::fs::read_to_string(CONFIG).expect("Failed to read config file");
        let cfg = toml::from_str::<toml::Table>(config.as_str()).unwrap();

        if cfg.contains_key("binsize") {
            let binsize = cfg.get("binsize")
                .expect("Config file must contain a [binsize] section")
                .as_table()
                .expect("[binsize] must be a table]");

            if let Some(toml::Value::Boolean(val)) = binsize.get("color") {
                self.color = *val;
            }

            if let Some(toml::Value::String(val)) = binsize.get("profile") {
                self.build_options.profile = val.clone();
            }

            if let Some(toml::Value::Array(val)) = binsize.get("output") {
                for s in val {
                    let str = s.as_str().expect("Output should be a string");

                    self.output.apply_pattern(str.try_into().unwrap());
                }
            }

            if let Some(toml::Value::String(val)) = binsize.get("file") {
                self.file = val.clone();
            }

            if let Some(toml::Value::String(val)) = binsize.get("filter") {
                self.filter = regex::Regex::new(val.as_str()).unwrap();
            }

            if let Some(toml::Value::String(val)) = binsize.get("ld-file") {
                self.ld_file = val.clone();
            }

            if let Some(toml::Value::String(val)) = binsize.get("sort") {
                match val.as_str() {
                    "asc" => {
                        self.symbols_sorting_order = Some(SortOrder::Ascending);
                    }
                    "desc" => {
                        self.symbols_sorting_order = Some(SortOrder::Descending);
                    }
                    _ => {
                        panic!("Invalid value for key 'sort': '{} (possible values: asc, desc)'", val);
                    }
                }
            }

            if let Some(toml::Value::Array(val)) = binsize.get("size-threshold") {
                self.size_threshold_yellow = val.get(0)
                    .expect("Missing first value for key 'size-threshold'")
                    .as_integer()
                    .expect("Values for key 'size-threshold' must be an integer")
                    as usize;

                self.size_threshold_red = val.get(1)
                    .expect("Missing second value for key 'size-threshold'")
                    .as_integer()
                    .expect("Values for key 'size-threshold' must be an integer")
                    as usize;
            }

            if let Some(toml::Value::Array(val)) = binsize.get("percentage-threshold") {
                self.percentage_threshold_yellow = val.get(0)
                    .expect("Missing first value for key 'size-threshold'")
                    .as_float()
                    .expect("Values for key 'size-threshold' must be a float")
                    as f32;

                self.percentage_threshold_red = val.get(1)
                    .expect("Missing second value for key 'size-threshold'")
                    .as_float()
                    .expect("Values for key 'size-threshold' must be a float")
                    as f32;
            }
        }
    }

    /// Parse command line arguments
    fn parse_args(&mut self) {
        let argp = args::ArgumentParser::new(
            vec![
                args::Argument::new_flag(
                    "help",
                    &["--help", "-h"],
                    "Display help message"
                ),
                args::Argument::new_flag(
                    "version",
                    &["--version", "-v"],
                    "Display version"
                ),
                args::Argument::new_value(
                    "profile",
                    &["--profile", "-p"],
                    &["PROFILE"],
                    "Cargo profile to build the project with"
                ),
                args::Argument::new_value(
                    "output",
                    &["--output", "-o"],
                    &["OUTPUT"],
                    "Comma separated list of output values with optional comma-separated list of columns"
                ),
                args::Argument::new_value(
                    "file",
                    &["--file"],
                    &["FILE"],
                    "Provide a path to compiled binary, skipping 'cargo build'"
                ),
                args::Argument::new_value(
                    "ld-memory-map",
                    &["--ld-memory-map", "-l"],
                    &["LD_PATH"],
                    "Path to ld script, containing MEMORY declaration"
                ),
                args::Argument::new_value(
                    "filter",
                    &["--filter", "-f"],
                    &["FILTER"],
                    "Filter symbol names by this value. Supports regex"
                ),
                args::Argument::new_flag(
                    "asc",
                    &["--asc", "-a"],
                    "Sort by symbol size in ascending order"
                ),
                args::Argument::new_flag(
                    "desc",
                    &["--desc", "-d"],
                    "Sort by symbol size in descending order"
                ),
                args::Argument::new_flag(
                    "color",
                    &["--color", "-c"],
                    "Add coloring to output"
                ),
                args::Argument::new_value(
                    "size-threshold",
                    &["--size-threshold"],
                    &["YELLOW", "RED"],
                    "Yellow & red size thresholds in bytes (default 200 500)"
                ),
                args::Argument::new_value(
                    "percentage-threshold",
                    &["--percentage-threshold"],
                    &["YELLOW", "RED"],
                    "Yellow & red size percentage thresholds (default 0.5 1.0)"
                ),
            ],
            args::UnexpectedArgumentPolicy::Crash
        );

        let parsed = argp.parse(std::env::args().skip(1));

        // FIXME: Is still needed?
        // if parsed.contains_arg("output") {
        //     self.output = Output::None as u8;
        // }

        for arg in parsed.args {
            match arg.name.as_str() {
                "help" => {
                    println!("binsize - utility to provide comprehensive information about symbol sizes in compiled binaries");
                    println!("Options:");
                    argp.print_help();
                    std::process::exit(0);
                }
                "version" => {
                    println!("binsize {}", VERSION);
                    std::process::exit(0);
                }
                "profile" => {
                    self.build_options.profile = arg.values.get(0)
                        .expect("Missing value for --profile")
                        .clone();
                }
                "output" => {
                    let val = arg.values.get(0).expect("Missing value for --output");
                    self.output.apply_pattern(val);
                }
                "file" => {
                    self.file = arg.values.get(0)
                            .expect("Missing value for --file")
                            .clone();
                }
                "filter" => {
                    self.filter = regex::Regex::new(arg.values.get(0)
                        .expect("Missing value for --filter")
                        .clone()
                        .as_str()
                    ).unwrap();
                }
                "ld-memory-map" => {
                    self.ld_file = arg.values.get(0)
                        .expect("Missing value for --ld-memory-map")
                        .clone();
                }
                "asc" => {
                    self.symbols_sorting_order = Some(SortOrder::Ascending);
                }
                "desc" => {
                    self.symbols_sorting_order = Some(SortOrder::Descending);
                }
                "color" => {
                    self.color = true;
                }
                "size-threshold" => {
                    self.size_threshold_yellow = arg.values.get(0)
                        .expect("Missing value YELLOW for --size-threshold")
                        .parse::<usize>()
                        .expect("yellow threshold must be a number");

                    self.size_threshold_red = arg.values.get(1)
                        .expect("Missing value RED for --size-threshold")
                        .parse::<usize>()
                        .expect("red threshold must be a number");
                }
                "percentage-threshold" => {
                    self.percentage_threshold_yellow = arg.values.get(0)
                        .expect("Missing value YELLOW for --percentage-threshold")
                        .parse::<f32>()
                        .expect("yellow threshold must be a float");

                    self.percentage_threshold_red = arg.values.get(1)
                        .expect("Missing value RED for --percentage-threshold")
                        .parse::<f32>()
                        .expect("red threshold must be a float");
                }
                arg => {
                    panic!("Unexpected argument: {}", arg);
                }
            }
        }
    }

    /// Load executable
    fn load_exe(&mut self) {
        // If file was specified (either via config of cmdline options)
        let path = if !self.file.is_empty() {
            std::path::PathBuf::from(&self.file)
        } else {
            // Run `cargo build` to get freshly compiled executable
            if let Err(stderr) = cargo::build(self.build_options.clone()) {
                println!("{}", stderr);
                std::process::exit(1);
            }

            // Run `cargo built --message-format=json` to gather info about artifacts produced
            // by build
            let artifacts = cargo::artifacts(self.build_options.clone());

            // Last artifact should be a `top crate` - executable or a library, for which
            // a binary would be generated
            let top_crate = artifacts.last()
                .expect("No top crate");

            // Extract path to binary
            top_crate.path.clone()
        };

        // Parse binary
        self.exe = exe::parse(&path)
            .expect("Failed to parse executable");
    }

    /// Helper function to push `str` into `header` and `padding` into `paddings`, only if output
    /// for this column/field is enabled, and adding color, only of color enabled
    ///
    /// # Arguments
    ///
    /// * `header` - Row that represents a header in a `Table`
    /// * `paddings` - Vec of `Padding`, stores padding for each column
    /// * `output_kind` - Kind of output (sections/segments/etc)
    /// * `field` - Column/field bitmask
    /// * `str` - Column name
    /// * `padding` - Column padding
    /// * `color_fn` - Function/closure to call, if colorful output is enabled
    ///
    /// # Example
    ///
    /// ```
    /// use OutputKind::*;
    /// use SymbolTableFields::*;
    ///
    /// let mut header = Row::default();
    /// let mut paddings = Vec::new();
    ///
    /// self.push_into_header_and_padding_color(
    ///     &mut header, &mut paddings,
    ///     Symbols, Size as u8,
    ///     "Size ", Padding::Right,
    ///     |s| {
    ///         s.push_attr(Attribute::TextBold);
    ///     }
    /// );
    ///
    /// let mut table = Table::with_header_and_padding(
    ///     header,
    ///     paddings.as_slice()
    /// );
    /// ```
    fn push_into_header_and_padding_color(
        &self,
        header:      &mut Row,
        paddings:    &mut Vec<Padding>,
        output_kind: OutputKind,
        field:       u8,
        str:         &str,
        padding:     Padding,
        color_fn:    impl Fn(&mut AttributeString)
    ) {
        if !self.output.field_enabled(output_kind, field) {
            return;
        }

        paddings.push(padding);

        self.push_into_row_color(header, output_kind, field, str, color_fn);
    }

    /// Helper function to push `str` into `row` only if output for this column/field is enabled,
    /// and adding color, only of color enabled
    ///
    /// # Arguments
    ///
    /// * `row` - Row to append column value to
    /// * `output_kind` - Kind of output (sections/segments/etc)
    /// * `field` - Column/field bitmask
    /// * `str` - Column value
    /// * `color_fn` - Function/closure to call, if colorful output is enabled
    ///
    /// # Example
    ///
    /// ```
    /// let table: Table = ...;
    /// let mut row = Row::default();
    ///
    /// self.push_into_row_color(
    ///     &mut row,
    ///     Symbols, Name as u8,
    ///     format!("{} ", sym.name).as_str(),
    ///     |s| {
    ///         s.push_attr(Attribute::TextBold)
    ///     }
    /// );
    ///
    /// table.push_row(row).unwrap();
    /// ```
    ///
    fn push_into_row_color(
        &self,
        row: &mut Row,
        output_kind: OutputKind,
        field: u8,
        str: &str,
        color_fn: impl Fn(&mut AttributeString)
    ) {
        if !self.output.field_enabled(output_kind, field) {
            return;
        }

        let mut attr_str = AttributeString::from(str);

        if self.color {
            color_fn(&mut attr_str);
        }

        row.push(attr_str);
    }

    /// Helper function to push `str` into `row` only if output for this column/field is enabled
    ///
    /// # Arguments
    ///
    /// * `row` - Row to append column value to
    /// * `output_kind` - Kind of output (sections/segments/etc)
    /// * `field` - Column/field bitmask
    /// * `str` - Column value
    ///
    /// # Example
    ///
    /// ```
    /// let table: Table = ...;
    /// let mut row = Row::default();
    ///
    /// self.push_into_row(
    ///     &mut row,
    ///     Symbols, Name as u8,
    ///     format!("{} ", sym.name).as_str()
    /// );
    ///
    /// table.push_row(row).unwrap();
    /// ```
    fn push_into_row(
        &self,
        row: &mut Row,
        output_kind: OutputKind,
        field: u8,
        str: &str
    ) {
        if !self.output.field_enabled(output_kind, field) {
            return;
        };

        row.push(AttributeString::from(str));
    }

    /// Dump symbols into a table
    fn dump_symbols(&mut self) {
        use OutputKind::*;
        use SymbolTableFields::*;
        
        if let Some(order) = &self.symbols_sorting_order {
            self.exe.sort_symbols(*order);
        }

        // Check if at least one symbol has a crate name
        let has_crate_names = self.exe.symbols.iter()
            .filter(|s| s.crate_name != "?").peekable().peek().is_some();

        // If no symbols have a crate name
        if !has_crate_names {
            // Disable `Crate` column in `Symbols` table
            self.output.field_disable(Symbols, Crate as u8);

            // Disable `Crates` table
            self.output.disable(Crates);
        }

        let total = self.exe.symbols.iter()
            .filter(|s| { matches!(self.filter.captures(&s.name), Some(_)) })
            .fold(0, |r, s| r + s.size);

        let mut header = Row::default();
        let mut paddings = Vec::new();

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Symbols, Size as u8,
            "Size ", Padding::Right,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Symbols, Percent as u8,
            "Percentage ", Padding::Right,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Symbols, Kind as u8,
            "Symbol Kind ", Padding::Right,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Symbols, Crate as u8,
            "Crate Name ", Padding::Right,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Symbols, Name as u8,
            "Symbol Name ", Padding::Left,
            color_header_fn
        );

        let mut table = Table::with_header_and_padding(header, paddings.as_slice());

        for sym in &self.exe.symbols {
            if sym.size == 0 {
                continue;
            }

            if matches!(self.filter.captures(&sym.name), Option::None) {
                continue;
            }

            let mut row = Row::default();

            self.push_into_row_color(
                &mut row,
                Symbols, Size as u8,
                format!("{} ", sym.size).as_str(),
                |s| {
                    if sym.size >= self.size_threshold_red {
                        s.push_attr(Attribute::ColorFgRed);
                    } else if sym.size >= self.size_threshold_yellow {
                        s.push_attr(Attribute::ColorFgYellow);
                    } else {
                        s.push_attr(Attribute::ColorFgGreen);
                    }
                }
            );

            let percentage = sym.size as f32 / (total as f32 / 100.0);

            self.push_into_row_color(
                &mut row,
                Symbols, Percent as u8,
                format!("{:.02}% ", percentage).as_str(),
                |s| {
                    if percentage >= self.percentage_threshold_red {
                        s.push_attr(Attribute::ColorFgRed);
                    } else if percentage >= self.percentage_threshold_yellow {
                        s.push_attr(Attribute::ColorFgYellow);
                    } else {
                        s.push_attr(Attribute::ColorFgGreen);
                    }
                }
            );

            self.push_into_row_color(
                &mut row,
                Symbols, Kind as u8,
                format!("{} ", sym.kind).as_str(),
                |s| {
                    match sym.kind {
                        SymbolKind::Function => s.push_attr(Attribute::ColorFgMagenta),
                        SymbolKind::Data     => s.push_attr(Attribute::ColorFgCyan),
                        SymbolKind::Unknown  => {},
                    }
                }
            );

            self.push_into_row(
                &mut row,
                Symbols, Crate as u8,
                format!("{} ", sym.crate_name).as_str()
            );

            self.push_into_row_color(
                &mut row,
                Symbols, Name as u8,
                format!("{} ", sym.name).as_str(),
                |s| {
                    s.push_attr(Attribute::TextBold)
                }
            );

            table.push_row(row).unwrap();
        }

        table.print();

        println!();
        println!("Total: {}", {
            let mut s = AttributeString::from(format!("{}", total).as_str());

            if self.color {
                s.push_attr(Attribute::TextBold);
            }

            s
        });
    }

    /// Dump crate sizes into a table
    fn dump_crates(&mut self) {
        use OutputKind::*;
        use CrateTableFields::*;

        println!();

        let mut crates = HashMap::new();

        for sym in self.exe.symbols.iter() {
            if crates.contains_key(&sym.crate_name) {
                *crates.get_mut(&sym.crate_name).unwrap() += sym.size;
            } else {
                crates.insert(&sym.crate_name, sym.size);
            }
        }

        let mut crates = crates.iter().collect::<Vec<_>>();

        if let Some(order) = self.symbols_sorting_order {
            crates.sort_by(|s1, s2|
                if match order {
                    SortOrder::Ascending  => s1.1 < s2.1,
                    SortOrder::Descending => s1.1 > s2.1
                } {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Greater
                }
            );
        }

        let mut header = Row::default();
        let mut paddings = Vec::new();

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Crates, Size as u8,
            "Crate Name ", Padding::Left,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Crates, Size as u8,
            "Size ", Padding::Right,
            color_header_fn
        );

        let mut table = Table::with_header_and_padding(header, paddings.as_slice());

        for (name, size) in crates {
            let mut row = Row::default();

            self.push_into_row(
                &mut row,
                Crates, Name as u8,
                ((*name).clone() + " ").as_str()
            );

            self.push_into_row(
                &mut row,
                Crates, Size as u8,
                format!("{} ", size).as_str()
            );
            
            table.push_row(row).unwrap();
        }

        table.print();
    }

    /// Dump sections into a table
    fn dump_sections(&mut self) {
        use OutputKind::*;
        use SectionTableFields::*;

        println!();

        let mut header = Row::default();
        let mut paddings = Vec::new();

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Sections, Name as u8,
            "Name ", Padding::Left,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Sections, Addr as u8,
            "Address ", Padding::Left,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Sections, Size as u8,
            "Size ", Padding::Right,
            color_header_fn
        );

        let mut table = Table::with_header_and_padding(header, paddings.as_slice());

        for section in self.exe.sections.iter() {
            let mut row = Row::default();

            self.push_into_row(
                &mut row,
                Sections, Name as u8,
                (section.name.clone() + " ").as_str()
            );

            self.push_into_row(
                &mut row,
                Sections, Addr as u8,
                format!("0x{:08x} ", section.addr).as_str()
            );

            self.push_into_row(
                &mut row,
                Sections, Size as u8,
                format!("{} ", section.size).as_str()
            );

            table.push_row(row).unwrap();
        }

        table.print();
    }

    /// Dump segments into a table, if `ld_file` is set
    fn dump_segments(&mut self) {
        use OutputKind::*;
        use SegmentTableFields::*;
        
        if self.ld_file.is_empty() {
            return;
        }

        println!();

        let mut header = Row::default();
        let mut paddings = Vec::new();

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Segments, Name as u8,
            "Name ", Padding::Left,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Segments, Addr as u8,
            "Address ", Padding::Left,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Segments, Used as u8,
            "Used ", Padding::Right,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Segments, Size as u8,
            "Size ", Padding::Right,
            color_header_fn
        );

        self.push_into_header_and_padding_color(
            &mut header, &mut paddings,
            Segments, Percent as u8,
            "Percentage ", Padding::Right,
            color_header_fn
        );

        let mut table = Table::with_header_and_padding(header, paddings.as_slice());

        // TODO: Shouldn't clone() ld_file
        let mut regions = link::MemoryRegion::from_file(&self.ld_file.clone().into())
            .expect("Failed to open LD file");

        link::MemoryRegion::use_segments_data(&mut regions, &self.exe.segments);

        for reg in regions.iter_mut() {
            let mut row = Row::default();

            self.push_into_row(
                &mut row,
                Segments, Name as u8,
                (reg.name.clone() + " ").as_str()
            );

            self.push_into_row(
                &mut row,
                Segments, Addr as u8,
                format!("0x{:08x} ", reg.origin).as_str()
            );

            self.push_into_row(
                &mut row,
                Segments, Used as u8,
                format!("{} ", reg.used).as_str()
            );

            self.push_into_row(
                &mut row,
                Segments, Size as u8,
                format!("{} ", reg.length).as_str()
            );

            self.push_into_row_color(
                &mut row,
                Segments, Percent as u8,
                format!("{:.02}% ", reg.used_percentage).as_str(),
                |s| {
                    if reg.used_percentage > 75.0 {
                        s.push_attr(Attribute::ColorFgRed);
                    } else if reg.used_percentage > 50.0 {
                        s.push_attr(Attribute::ColorFgYellow);
                    } else {
                        s.push_attr(Attribute::ColorFgGreen);
                    }
                }
            );

            table.push_row(row).unwrap()
        }

        table.print();
    }

    /// Run whole application
    /// Will parse cmdline arguments, config, and output all configured tables
    ///
    /// # Example
    /// ```
    /// Binsize::new().run();
    /// ```
    fn run(&mut self) {
        self.parse_config();
        self.parse_args();

        if !self.output.any_enabled() {
            self.output.enable(OutputKind::Symbols);
        }

        self.load_exe();

        if self.output.enabled(OutputKind::Symbols) {
            self.dump_symbols();
        }

        if self.output.enabled(OutputKind::Crates) {
            self.dump_crates();
        }

        if self.output.enabled(OutputKind::Sections) {
            self.dump_sections();
        }

        if self.output.enabled(OutputKind::Segments) {
            self.dump_segments();
        }
    }
}

fn main() {
    Binsize::new().run();
}
