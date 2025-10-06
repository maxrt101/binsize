//! # `binsize::args`
//!
//! Implements simple command-line arguments parser
//!

use std::collections::HashMap;
use crate::table::{Padding, Table};

/// Represents argument types
#[derive(PartialEq)]
pub enum ArgumentKind {
    /// Flag without trailing value
    Flag,

    /// Flag with value
    Value,
}

/// Represents argument metadata
///
/// Example:
/// ```
/// args::Argument::new_flag(
///     "help",
///     &["--help", "-h"],
///     "Display help message"
/// )
/// args::Argument::new_value(
///     "profile",
///     &["--profile", "-p"],
///     &["PROFILE"],
///     "Cargo profile to build the project with"
/// )
/// ```
pub struct Argument {
    /// Argument name - used after `ArgumentParser::parse()` to distinguish parsed arguments
    name: String,

    /// Argument kind - either a flag without value, of with
    kind: ArgumentKind,

    /// Which keys for this argument are used (e.g. `--flag`, `-f`, etc.)
    keys: Vec<String>,

    /// Values and their names (e.g. `--flag1 VALUE` or `--flag2 VALUE1 VALUE2`)
    values: Vec<String>,

    /// Description used for `ArgumentParser::print_help()`
    description: String,
}

impl Argument {
    /// Creates new argument
    pub fn new(name: &str, kind: ArgumentKind, keys: &[&str], values: &[&str], description: &str) -> Self {
        Self {
            name: name.to_string(),
            kind,
            keys: keys.iter().map(|a| a.to_string()).collect(),
            values: values.iter().map(|a| a.to_string()).collect(),
            description: description.to_string(),
        }
    }

    /// Creates new flag. Omits unnecessary arguments from `new()`
    pub fn new_flag(name: &str, keys: &[&str], description: &str) -> Self {
        Self::new(name, ArgumentKind::Flag, keys, &[], description)
    }

    /// Creates new argument with value. Omits unnecessary arguments from `new()`
    pub fn new_value(name: &str, keys: &[&str], values: &[&str], description: &str) -> Self {
        Self::new(name, ArgumentKind::Value, keys, values, description)
    }
}

/// Represents a parsed argument
pub struct ParsedArgument {
    /// Argument name from `Argument`
    pub name: String,

    /// Parsed values (empty for Flag)
    pub values: Vec<String>,
}

/// Encapsulates parsed argument list, and unrecognized arguments
pub struct ParsedArguments {
    /// Parsed arguments
    pub args: Vec<ParsedArgument>,

    /// Unrecognized arguments
    pub leftover: Vec<String>,
}

/// Defines policy on encountering unexpected argument
#[allow(dead_code)]
pub enum UnexpectedArgumentPolicy {
    /// Ignore the argument
    Ignore,

    /// Save it into `ParsedArguments::leftover`
    Save,

    /// Panic
    Crash,
}

/// Argument parser context
///
/// Example:
/// ```
/// let argp = ArgumentParser::new(
///     vec![
///         Argument::new_flag(
///             "help",
///             &["--help", "-h"],
///             "Display help message"
///         ),
///         Argument::new_value(
///             "profile",
///             &["--profile", "-p"],
///             &["PROFILE"],
///             "Cargo profile to build the project with"
///         ),
///     ],
///     UnexpectedArgumentPolicy::Crash
/// );
///
/// for arg in argp.parse(std::env::args().skip(1)).args {
///     match arg.name.as_str() {
///         "help" => {
///             println!("Usage: program [OPTIONS]");
///             println!("Options:");
///             argp.print_help();
///             std::process::exit(0);
///         }
///         "profile" => {
///             profile = arg.values.get(0)
///                 .expect("Missing value for --profile")
///                 .clone();
///         }
///         arg => {
///             panic!("Unexpected argument: {}", arg);
///         }
///     }
/// }
///
/// ```
pub struct ArgumentParser {
    /// Argument metadata
    args: HashMap<String, Argument>,

    /// Map of `Argument::keys` to `Argument::name`
    keymap: HashMap<String, String>,

    /// Used in `print_help()` to print arguments in order, that they were declared
    order: Vec<String>,

    /// Policy on unknown/unrecognized arguments
    unknown_argument_policy: UnexpectedArgumentPolicy
}

impl ArgumentParser {
    /// Creates new `ArgumentParser`
    pub fn new(args: Vec<Argument>, unknown_argument_policy: UnexpectedArgumentPolicy) -> Self {
        let mut keymap = HashMap::new();
        let mut order = Vec::new();

        for arg in args.iter() {
            order.push(arg.name.clone());
            for key in &arg.keys {
                keymap.insert(key.clone(), arg.name.clone());
            }
        }

        let args = args.into_iter().map(|a| (a.name.clone(), a)).collect();

        Self { args, keymap, order, unknown_argument_policy }
    }

    /// Prints help message for each argument
    pub fn print_help(&self) {
        let mut table = Table::with_empty_header_and_padding(vec![
            Padding::None, Padding::Left, Padding::None, Padding::Left
        ]);

        for name in self.order.iter() {
            let arg = &self.args[name];

            table.push_row([
                // 4 spaces for prettiness
                "    ",

                // Join all argument keys + argument values into single column in this row
                (arg.keys.join(", ") + " " + arg.values.join(" ").as_str()).as_str(),

                // Delimiter between argument keys + values and description
                " - ",

                // Description
                arg.description.as_str()
            ].into()).unwrap();
        }

        table.print();
    }

    /// Handles expected arguments
    fn handle_expected(&self, result: &mut ParsedArguments, arg: String, args: &mut impl Iterator<Item = String>) {
        // This `.unwrap()` here should panic, as this function is called only when the argument
        // key was already confirmed to be declared and known in this parser
        let arg = self.args.get(&self.keymap[&arg]).unwrap();

        match arg.kind {
            ArgumentKind::Flag => {
                result.args.push(ParsedArgument {
                    name: arg.name.clone(),
                    values: vec![],
                });
            }
            ArgumentKind::Value => {
                result.args.push(ParsedArgument {
                    name: arg.name.clone(),
                    values: {
                        let mut values = Vec::new();

                        // Consume all expected values
                        for value in arg.values.iter() {
                            values.push(args.next().expect(format!("Expected value '{}' for argument '{}'", value, arg.name).as_str()));
                        }

                        values
                    },
                });
            }
        }
    }

    /// Handles unexpected arguments
    fn handle_unexpected(&self, result: &mut ParsedArguments, arg: String) {
        match self.unknown_argument_policy {
            UnexpectedArgumentPolicy::Ignore => {
                // noop
            }
            UnexpectedArgumentPolicy::Save => {
                result.leftover.push(arg);
            }
            UnexpectedArgumentPolicy::Crash => {
                panic!("Unexpected argument: {}", arg);
            }
        }
    }

    /// Performs actual parsing of the arguments.
    /// Arguments are passed using an iterator
    pub fn parse(&self, mut args: impl Iterator<Item = String>) -> ParsedArguments {
        let mut result = ParsedArguments { args: Vec::new(), leftover: Vec::new() };

        while let Some(arg) = args.next() {
            if self.keymap.contains_key(&arg) {
                self.handle_expected(&mut result, arg, &mut args);
            } else {
                self.handle_unexpected(&mut result, arg);
            }
        }

        result
    }
}
