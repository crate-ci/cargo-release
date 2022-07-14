use std::env::VarError;
use std::io::Error as IOError;
use std::path::PathBuf;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

use cargo_metadata::Error as CargoMetaError;
use quick_error::quick_error;
use fancy_regex::Error as RegexError;
use semver::Error as SemVerError;
use toml_edit::easy::de::Error as TomlDeError;
use toml_edit::easy::ser::Error as TomlSerError;
use toml_edit::TomlError as TomlEditError;

quick_error! {
    #[derive(Debug)]
    pub enum FatalError {
        IOError(err: IOError) {
            from()
            source(err)
            display("IO Error: {}", err)
        }
        FileNotFound(filename: PathBuf){
            display("Unable to find file {} to perform replace", filename.display())
        }
        InvalidTomlData(err: TomlDeError) {
            display("Invalid TOML file format: {}", err)
            from()
            source(err)
        }
        InvalidTomlFileFormat(err: TomlSerError) {
            display("Invalid TOML file format: {}", err)
            from()
            source(err)
        }
        InvalidTomlEditFileFormat(err: TomlEditError) {
            display("Invalid TOML file format: {}", err)
            from()
            source(err)
        }
        InvalidCargoMetaFileFormat(err: CargoMetaError) {
            display("Invalid Cargo file format: {}", err)
            from()
            source(err)
        }
        InvalidCargoFileFormat(msg: String) {
            display("Invalid TOML file format: {}", msg)
        }
        InvalidCargoConfigKeys {
            display("Invalid cargo-release config item found")
        }
        SemVerError(err: SemVerError) {
            from()
            source(err)
            display("SemVerError {}", err)
        }
        IgnoreError(err: ignore::Error) {
            from()
            source(err)
            display("ignore-pattern {}", err)
        }
        Utf8Error(err: Utf8Error) {
            from()
            source(err)
            display("Utf8Error {}", err)
        }
        FromUtf8Error(err: FromUtf8Error) {
            from()
            source(err)
            display("FromUtf8Error {}", err)
        }
        IndexError(err: crates_index::Error) {
            from()
            source(err)
            display("{}", err)
        }
        Git2Error(err: git2::Error) {
            from()
            source(err)
            display("{}", err)
        }
        NoPackage {
            display("No package in manifest file")
        }
        PackageListFailed(manifest: std::path::PathBuf, err: String) {
            display("Failed to get package content for {}: {}", manifest.display(), err)
        }
        InvalidReleaseLevel(level: String) {
            display("Unsupported release level {}, only major, minor and patch are supported", level)
        }
        UnsupportedPrereleaseVersionScheme {
            display("This version scheme is not supported by cargo-release. Use format like `pre`, `dev` or `alpha.1` for prerelease symbol")
        }
        UnsupportedVersionReq(req: String) {
            display("Support for modifying {} is currently unsupported", req)
        }
        ReplacerConfigError {
            display("Insufficient replacer config: file, search and replace are required.")
        }
        ReplacerRegexError(err: RegexError) {
            from()
            source(err)
            display("RegexError {}", err)
        }
        ReplacerMinError(pattern: String, req: usize, actual: usize) {
            display("For `{}`, at least {} replacements expected, found {}", pattern, req, actual)
        }
        ReplacerMaxError(pattern: String, req: usize, actual: usize) {
            display("For `{}`, at most {} replacements expected, found {}", pattern, req, actual)
        }
        VarError(err: VarError) {
            from()
            source(err)
            display("Environment Variable Error: {}", err)
        }
        GitBinError {
            display("git is not found. git is required for cargo-release workflow.")
        }
        PublishTimeoutError {
            display("Timeout waiting for crate to be published.")
        }
        DependencyVersionConflict {
            display("Dependency is configured to conflict with new version")
        }
    }
}
