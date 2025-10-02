```
░▒▓███████▓▒░░▒▓█▓▒░▒▓███████▓▒░ ░▒▓███████▓▒░▒▓█▓▒░▒▓████████▓▒░▒▓████████▓▒░ 
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░      ░▒▓█▓▒░▒▓█▓▒░        
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░    ░▒▓██▓▒░░▒▓█▓▒░        
░▒▓███████▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░░▒▓██████▓▒░░▒▓█▓▒░  ░▒▓██▓▒░  ░▒▓██████▓▒░   
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░      ░▒▓█▓▒░▒▓█▓▒░░▒▓██▓▒░    ░▒▓█▓▒░        
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░      ░▒▓█▓▒░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░        
░▒▓███████▓▒░░▒▓█▓▒░▒▓█▓▒░░▒▓█▓▒░▒▓███████▓▒░░▒▓█▓▒░▒▓████████▓▒░▒▓████████▓▒░ 
```

`binsize` is a command-line utility that provides analysis of size/memory usage of rust  
binaries. It was inspired by `cargo-bloat`, but with a different approach to retrieving  
symbols. Main difference is that `binsize` parses *all* symbols (both functions and  
data/constants), except for those with a size of 0. `binsize` also provides colored output,  
memory region usage (if provided with a path to linker script that has a `MEMORY` definition)  

Note: file, that is being analyzed, must have `.symtab` section, otherwise `binsize` won't  
be able to parse exported symbols. So don't strip your binaries, if you want this to work.  

Note: this is only a prototype, bugs are expected.  

## Installation

Run `cargo install binsize`.  
Or build manually: clone the repo, run `cargo build`, enjoy :)  

## Usage

Navigate to you project (folder containing `Cargo.toml`) and run:  

```shell
$ binsize
```

You should see a table with columns:  
`Size`        - Size of symbol in bytes  
`Percentage`  - Size of symbol as a percentage of size of all symbols combined  
`Symbol Kind` - Symbols Type, either `FUNC` of `DATA` (if symbol kind couldn't be parsed, it'll display `UNK`)  
`Crate Name`  - Crate name derived from demangled symbol name. Isn't super accurate for now  
`Symbol Name` - Demangled symbol name  

If you want to analyze artifact, produced with a different cargo profile, use `--profile`/`-p`
flag:  

```shell
$ binsize -p release
```

If you want to skip building through cargo, or want to analyze some other binary, pass a path
to said file using `--file`:  

```shell
$ binsize --file ~/projects/super-cool-project/target/release/super-cool-project
```

If you want to enable colored output, use `--color`/`-c` flag:  

```shell
$ binsize -c
```

With enabled colorful output, you'll see that `Size` & `Percentage` columns became green,  
yellow or red. This serves as a visual indicator, of a symbol being too large. Threshold for  
symbol's size/percentage to become yellow/red can be overridden using `--size-threshold` and  
`--percentage-threshold`:

```shell
$ binsize --percentage-threshold 1.2 5.0 --size-threshold 500 1200
```

If some symbol has too big of a name, and it got trimmed, you can use `--width`/`-w` to increase  
(or decrease) maximal width of symbol name:  

```shell
$ binsize -c 120
```

If you want to sort symbols by size, use `--asc`/`-a` or `--desc`/`-d`:  

```shell
$ binsize --asc
```

For embedded projects, I really like GCC's `--print-memory-usage` linker flag, but using rust and  
cargo, I found it pretty hard to display the information about memory region usage (FLASH/RAM/etc.).  
So `binsize` provides a way to get that information, albeit not without user input. To get  
memory region usage, you must pass a path to linker script, which has a `MEMORY` declaration,  
like this:  

```ld
MEMORY
{
  FLASH : ORIGIN = 0x8000000,  LENGTH = 64K
  RAM   : ORIGIN = 0x20000000, LENGTH = 8K
}
```

The `--ld-memory-map`/`-l` is used to pass the path:  

```shell
$ binsize -l boards/stm32l051/memory.x
```

After running this, you'll get a table at the very bottom of the output with columns:  
`Name`       - Name of memory region as defined in linker script  
`Address`    - Base of a region. Corresponds to ORIGIN in linker script  
`Used`       - How much of region is used, calculated using info from parsed program headers  
`Size`       - Full size of a region. Corresponds to LENGTH in linker script  
`Percentage` - Percentage of used against full size  

Note: If memory region ORIGIN is not in hexadecimal, or LENGTH is not declared as  
`<base 10 int>K`, linker script parsing will fail, this is known limitation right now  

## Config

`binsize` also support persistent configuration stored in `.cargo/binsize.toml`  
Here's an example of such config:  

```toml
[binsize]
color = true
profile = "release"
output = ["symbols", "segments"]
file = "target/release/app"
ld-file = "boards/stm32l051/memory.x"
filter = "std"
sort = "asc"
width = 100
size-threshold = [5000, 10000]
percentage-threshold = [0.5, 1.0]
```

Note: command line arguments will override config values  
