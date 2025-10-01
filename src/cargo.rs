//! # `binsize::cargo`
//!
//! Implements interface to trigger `cargo build` and parse it's json output to retrieve
//! a list of built artifacts
//!

use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Command;

/// Represents build options passed to `cargo build`
/// TODO: Add an ability to pass any option
#[derive(Clone)]
pub struct BuildOptions {
    /// Build profile
    pub profile: String,

    /// Message format for target artifacts parsing
    pub message_format: String,
}

impl BuildOptions {
    /// Creates new build options
    pub fn new(profile: String, message_format: String) -> Self {
        Self { profile, message_format }
    }

    /// Overrides profile value, consuming `BuildOptions` and returning a new one
    pub fn profile(mut self, profile: &str) -> Self {
        self.profile = profile.to_string();
        self
    }

    /// Overrides message_format value, consuming `BuildOptions` and returning a new one
    pub fn message_format(mut self, message_format: &str) -> Self {
        self.message_format = message_format.to_string();
        self
    }

    /// Builds options into vector of command-line arguments to cargo
    pub fn args(&self) -> Vec<String> {
        let mut args = vec!["build".to_string()];

        if self.profile != "" {
            args.push("--profile".to_string());
            args.push(self.profile.clone());
        }

        if self.message_format != "" {
            args.push(format_args!("--message-format={}", self.message_format).to_string());
        }

        args
    }
}

impl Default for BuildOptions {
    fn default() -> Self {
        // Default profile if allways `dev`
        Self::new("dev".to_string(), "".to_string())
    }
}


/// Kind of build artifact
#[derive(PartialEq, Debug)]
pub enum BuildArtifactKind {
    Binary,
    Library,
    DynamicLibrary
}

impl TryFrom<&str> for BuildArtifactKind {
    type Error = ();

    /// Converts from value in `cargo build` json report
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "bin"              => Ok(Self::Binary),
            "lib"   | "rlib"   => Ok(Self::Library),
            "dylib" | "cdylib" => Ok(Self::DynamicLibrary),
            _                  => Err(()),
        }
    }
}

/// Represents information about a build artifact
pub struct BuildArtifact {
    pub kind: BuildArtifactKind,
    pub name: String,
    pub path: PathBuf
}

impl BuildArtifact {
    /// Creates new `BuildArtifact`
    pub fn new(kind: BuildArtifactKind, name: String, path: PathBuf) -> Self {
        Self { kind, name, path }
    }
}

impl TryFrom<(&str, &str, &str)> for BuildArtifact {
    type Error = ();

    /// Convert from 3 &str's, used by `cargo build` json parser
    fn try_from(value: (&str, &str, &str)) -> Result<Self, Self::Error> {
        let (crate_type, target, path) = value;

        Ok(BuildArtifact::new(
            BuildArtifactKind::try_from(crate_type)?,
            target.replace("-", "_"),
            std::path::PathBuf::from(path)
        ))
    }
}

impl Debug for BuildArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {} {:?}", self.kind, self.name, self.path)
    }
}

/// Run `cargo-build` with given build options
pub fn build(opt: BuildOptions) -> Result<(), String> {
    let cargo_build = Command::new("cargo")
        .args(opt.args())
        .output()
        .expect("cargo build failed");

    // Return cargo error output through Result:Err
    if !cargo_build.status.success() {
        return Err(String::from_utf8_lossy(&cargo_build.stderr).to_string().clone());
    }

    Ok(())
}

/// Parse `cargo-build` json output, and produce a list or build artifacts
pub fn artifacts(opt: BuildOptions) -> Vec<BuildArtifact> {
    // Won't actually build the project, because of `--message-format=json` (or at least I think it won't)
    let cargo_build_info = Command::new("cargo")
        .args(opt.message_format("json").args())
        .output()
        .expect("cargo build failed");

    if !cargo_build_info.status.success() {
        panic!("cargo build failed");
    }

    let mut artifacts = Vec::new();

    // Heavily inspired by cargo-bloat
    for line in String::from_utf8_lossy(&cargo_build_info.stdout).lines() {
        let build = json::parse(line).expect("invalid json output from cargo");

        if let Some(target) = build["target"]["name"].as_str() {
            if !build["filenames"].is_null() {
                let filenames = build["filenames"].members();
                let crate_types = build["target"]["crate_types"].members();

                for (path, crate_type) in filenames.zip(crate_types) {
                    let artifact = BuildArtifact::try_from((
                        crate_type.as_str().unwrap(),
                        target,
                        path.as_str().unwrap()
                    ));

                    if artifact.is_ok() {
                        artifacts.push(artifact.unwrap());
                    }
                }
            }
        }
    }

    artifacts
}
