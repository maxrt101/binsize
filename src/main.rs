//! # `binsize`
//!
//! `binsize` is a command-line utility that provides analysis of size/memory usage of rust
//! binaries. It was inspired by `cargo-bloat`, but with a different approach to retrieving
//! symbols. Main difference is that `binsize` parses *all* symbols (both functions and
//! data/constants), except for those with a size of 0. `binsize` also provides colored output,
//! memory region usage (if provided with a path to linker script that has a `MEMORY` definition)
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
//! You should see a table with columns:
//! `Size`        - Size of symbol in bytes
//! `Percentage`  - Size of symbol as a percentage of size of all symbols combined
//! `Symbol Kind` - Symbols Type, either `FUNC` of `DATA` (if symbol kind couldn't be parsed, it'll
//!                 display `UNK`)
//! `Crate Name`  - Crate name derived from demangled symbol name. Isn't super accurate for now
//! `Symbol Name` - Demangled symbol name
//!
//! If you want to analyze artifact, produced with a different cargo profile, use `--profile`/`-p`
//! flag:
//!
//! ```rust,ignore
//! $ binsize -p release
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
//! $ binsize -c
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
//! If some symbol has too big of a name, and it got trimmed, you can use `--width`/`-w` to increase
//! (or decrease) maximal width of symbol name:
//!
//! ```rust,ignore
//! $ binsize -c 120
//! ```
//!
//! If you want to sort symbols by size, use `--asc`/`-a` or `--desc`/`-d`:
//!
//! ```rust,ignore
//! $ binsize --asc
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
//! $ binsize -l boards/stm32l051/memory.x
//! ```
//!
//! After running this, you'll get a table at the very bottom of the output with columns:
//! `Name`       - Name of memory region as defined in linker script
//! `Address`    - Base of a region. Corresponds to ORIGIN in linker script
//! `Used`       - How much of region is used, calculated using info from parsed program headers
//! `Size`       - Full size of a region. Corresponds to LENGTH in linker script
//! `Percentage` - Percentage of used against full size
//!
//! Note: If memory region ORIGIN is not in hexadecimal, or LENGTH is not declared as
//! `<base 10 int>K`, linker script parsing will fail, this is known limitation right now
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
//! file = "target/release/app"
//! ld-file = "boards/stm32l051/memory.x"
//! filter = "std"
//! sort = "asc"
//! width = 100
//! size-threshold = [5000, 10000]
//! percentage-threshold = [0.5, 1.0]
//! ```
//!

use crate::table::{Padding, Row, Table};
use crate::attr_str::{Attribute, AttributeString};
use crate::cargo::BuildOptions;
use crate::exe::{ExecutableInfo, SymbolKind};
use crate::util::SortOrder;

mod cargo;
mod exe;
mod args;
mod table;
mod util;
mod attr_str;
mod link;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CONFIG: &str = ".cargo/binsize.toml";

/// `binsize` Application
struct Binsize {
    /// Cargo build options
    build_options: BuildOptions,

    /// Filter for symbol names
    filter: Option<String>,

    /// Linker script path with `MEMORY` declaration
    ld_file: Option<String>,

    /// File to parse (if `None` - will try to extract file from `cargo build`)
    file: Option<String>,

    /// Colorful output toggle
    color: bool,

    /// Max width of column in `Table`
    width: usize,

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

    /// Executable info
    exe: ExecutableInfo,
}

impl Binsize {
    fn new() -> Self {
        Self {
            build_options: Default::default(),
            filter: None,
            ld_file: None,
            file: None,
            color: false,
            width: 50,
            symbols_sorting_order: None,
            percentage_threshold_yellow: 0.5,
            percentage_threshold_red: 1.0,
            size_threshold_yellow: 200,
            size_threshold_red: 500,
            exe: ExecutableInfo::default(),
        }
    }

    fn parse_config(&mut self) {
        if matches!(std::fs::exists(CONFIG), Ok(true)) {
            let config = std::fs::read_to_string(CONFIG).expect("Failed to read config file");
            let cfg = toml::from_str::<toml::Table>(config.as_str()).unwrap();

            if cfg.contains_key("binsize") {
                let binsize = cfg.get("binsize")
                    .unwrap()
                    .as_table()
                    .unwrap();

                if let Some(toml::Value::Boolean(val)) = binsize.get("color") {
                    self.color = *val;
                }

                if let Some(toml::Value::String(val)) = binsize.get("profile") {
                    self.build_options.profile = val.clone();
                }

                if let Some(toml::Value::String(val)) = binsize.get("file") {
                    self.file = Some(val.clone());
                }

                if let Some(toml::Value::String(val)) = binsize.get("filter") {
                    self.filter = Some(val.clone());
                }

                if let Some(toml::Value::String(val)) = binsize.get("ld-file") {
                    self.ld_file = Some(val.clone());
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

                if let Some(toml::Value::Integer(val)) = binsize.get("width") {
                    self.width = *val as usize;
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
    }

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
                    "file",
                    &["--file"],
                    &["FILE"],
                    "Provide a path to compiled binary, skipping 'cargo build'"
                ),
                args::Argument::new_value(
                    "filter",
                    &["--filter", "-f"],
                    &["FILTER"],
                    "Filter symbol names by this value"
                ),
                args::Argument::new_value(
                    "width",
                    &["--width", "-w"],
                    &["WIDTH"],
                    "Max width of symbol name (default: 80)"
                ),
                args::Argument::new_value(
                    "ld-memory-map",
                    &["--ld-memory-map", "-l"],
                    &["LD_PATH"],
                    "Path to ld script, containing MEMORY declaration"
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

                // Make filter comma separated + any()-like upon checking OR make it a regex
                // + -a/--asc
                // + -d/--desc
                //   --sym/--symbols
                //   --sec/--sections
                //   --seg/--segments
                // + -c/--color
                //   -t/--table size,percent,kind,crate,name
            ],
            args::UnexpectedArgumentPolicy::Crash
        );

        for arg in argp.parse(std::env::args().skip(1)).args {
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
                "file" => {
                    self.file = Some(
                        arg.values.get(0)
                            .expect("Missing value for --file")
                            .clone()
                    );
                }
                "profile" => {
                    self.build_options.profile = arg.values.get(0)
                        .expect("Missing value for --profile")
                        .clone();
                }
                "filter" => {
                    self.filter = Some(arg.values.get(0)
                        .expect("Missing value for --filter")
                        .clone());
                }
                "width" => {
                    self.width = arg.values.get(0)
                        .expect("Missing value for --width")
                        .parse::<usize>()
                        .expect("width must be a number");
                }
                "ld-memory-map" => {
                    self.ld_file = Some(arg.values.get(0)
                        .expect("Missing value for --ld-memory-map")
                        .clone());
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

    fn load_exe(&mut self) {
        let path = if self.file.is_some() {
            // TODO: Shouldn't clone()???
            std::path::PathBuf::from(self.file.clone().unwrap())
        } else {
            if let Err(stderr) = cargo::build(self.build_options.clone()) {
                println!("{}", stderr);
                std::process::exit(1);
            }

            let artifacts = cargo::artifacts(self.build_options.clone());

            let top_crate = artifacts.last()
                .expect("No top crate");

            top_crate.path.clone()
        };

        self.exe = exe::parse(&path)
            .expect("Failed to parse executable");
    }

    fn dump_symbols(&mut self) {
        if let Some(order) = &self.symbols_sorting_order {
            self.exe.sort_symbols(*order);
        }

        // TODO: Shouldn't clone() filter???
        let total = self.exe.symbols.iter()
            .filter(|s| { !(!self.filter.is_none() && !s.name.contains(&self.filter.clone().unwrap())) })
            .fold(0, |r, s| r + s.size);

        let mut table = Table::with_header_and_padding(
            Row::from(["Size ", "Percentage ", "Symbol Kind ", "Crate Name ", "Symbol Name "]).map(|s| {
                let mut s = s.clone();
                if self.color {
                    s.push_attr(Attribute::TextBold);
                }
                s
            }),
            &[Padding::Right, Padding::Right, Padding::Right, Padding::Right, Padding::Left]
        );

        table.set_max_width(self.width);

        for sym in &self.exe.symbols {
            if sym.size != 0 {
                if !self.filter.is_none() && !sym.name.contains(&self.filter.clone().unwrap()) {
                    continue;
                }

                let mut row = Row::default();

                row.push({
                    let mut s: AttributeString = format!("{} ", sym.size).into();

                    if self.color {
                        if sym.size >= self.size_threshold_red {
                            s.push_attr(Attribute::ColorFgRed);
                        } else if sym.size >= self.size_threshold_yellow {
                            s.push_attr(Attribute::ColorFgYellow);
                        } else {
                            s.push_attr(Attribute::ColorFgGreen);
                        }
                    }

                    s
                });

                row.push({
                    let percentage = sym.size as f32 / (total as f32 / 100.0);
                    let mut s: AttributeString = format!("{:.02}% ", percentage).into();

                    if self.color {
                        if percentage >= self.percentage_threshold_red {
                            s.push_attr(Attribute::ColorFgRed);
                        } else if percentage >= self.percentage_threshold_yellow {
                            s.push_attr(Attribute::ColorFgYellow);
                        } else {
                            s.push_attr(Attribute::ColorFgGreen);
                        }
                    }

                    s
                });

                row.push({
                    let mut s: AttributeString = format!("{} ", sym.kind).into();

                    if self.color {
                        match sym.kind {
                            SymbolKind::Function => s.push_attr(Attribute::ColorFgMagenta),
                            SymbolKind::Data     => s.push_attr(Attribute::ColorFgCyan),
                            SymbolKind::Unknown  => {},
                        }
                    }

                    s
                });

                row.push(format!("{} ", sym.crate_name).into());

                row.push({
                    let mut s: AttributeString = format!("{} ", sym.name).into();

                    if self.color {
                        s.push_attr(Attribute::TextBold)
                    }

                    s
                });

                table.push_row(row).unwrap();
            }
        }

        println!("Symbols:");
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

    fn dump_sections(&mut self) {
        println!();

        let mut table = Table::with_header_and_padding(
            Row::from(["Name ", "Address ", "Size "]).map(|s| {
                let mut s = s.clone();
                if self.color {
                    s.push_attr(Attribute::TextBold);
                }
                s
            }),
            &[Padding::Left, Padding::Left, Padding::Right],
        );

        for section in self.exe.sections.iter() {
            table.push_row([
                section.name.clone(),
                format!("0x{:08x} ", section.addr),
                format!("{} ", section.size)
            ].into()).unwrap();
        }

        println!("Sections:");
        table.print();
    }

    fn dump_segments(&mut self) {
        if !self.ld_file.is_none() {
            println!();

            let mut table = Table::with_header_and_padding(
                Row::from(["Name ", "Address ", "Used ", "Size ", "Percentage "]).map(|s| {
                    let mut s = s.clone();
                    if self.color {
                        s.push_attr(Attribute::TextBold);
                    }
                    s
                }),
                &[Padding::Left, Padding::Left, Padding::Right, Padding::Right, Padding::Right],
            );

            // TODO: Shouldn't clone() ld_file
            let mut regions = link::MemoryRegion::from_file(&self.ld_file.clone().unwrap().into()).unwrap();

            link::MemoryRegion::use_segments_data(&mut regions, &self.exe.segments);

            for reg in regions.iter_mut() {
                let mut row = Row::default();

                row.push(reg.name.clone().into());
                row.push(format!("0x{:08x} ", reg.origin).into());
                row.push(format!("{} ", reg.used).into());
                row.push(format!("{} ", reg.length).into());
                row.push((format!("{:.02}% ", reg.used_percentage), {
                    if self.color {
                        if reg.used_percentage > 75.0 {
                            &[Attribute::ColorFgRed]
                        } else if reg.used_percentage > 50.0 {
                            &[Attribute::ColorFgYellow]
                        } else {
                            &[Attribute::ColorFgGreen]
                        }
                    } else {
                        &[Attribute::TextReset]
                    }
                }).into());

                table.push_row(row).unwrap()
            }

            println!("Region usage (based on LOAD segments and linker memory map):");
            table.print();
        }
    }

    fn run(&mut self) {
        self.parse_config();
        self.parse_args();

        self.load_exe();

        self.dump_symbols();
        self.dump_sections();
        self.dump_segments();
    }
}


fn main() {
    Binsize::new().run();
}
