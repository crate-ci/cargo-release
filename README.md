# cargo release

Features
- Ensure you are in a good state for release, including:
  - Right branch
  - Up-to-date with remote
  - Clean tree
- Supports workspaces using cargo's native flags, like `--workspace`, `--exclude` and `--package`
  - Updates dependent crates in workspace when changing version
  - Change detection to help guide in what crates might not need a release
  - Optionally share commits
- Handles `cargo publish`, tagging, and pushing
- Pre-release search and replace for custom version updates, including
  - Updating changelogs
  - Update tags in `Dockerfile`s
- Pre-release hook for extra customization, including
  - [CHANGELOG generation](https://github.com/orhun/git-cliff)

## Install

Current release: 0.21.3

`cargo install cargo-release`

## Usage

`cargo release [level]`

* See the [reference](docs/reference.md) for more on `level`, other CLI
  arguments, and configuration file format.
* See also the [FAQ](docs/faq.md) for help in figuring out how to adapt
  cargo-release to your workflow.

### Prerequisite

* Your project should be managed by git.

### Dry run

By default, `cargo-release` runs in dry-run mode so you can safely run it and
verify what it will do.
- Increase the logging level with each additional `-v` to get more details
- Speed up dry-run by skipping `cargo-publish`s verify step with `--no-verify`

Once you are ready, pass the `--execute` flag.

## Related tools

- [release-pr Action](https://github.com/cargo-bins/release-pr)
- [cargo-smart-release](https://github.com/Byron/gitoxide/tree/main/cargo-smart-release)
- [cargo-set-version](https://github.com/killercup/cargo-edit)
- [cargo-unleash](https://github.com/paritytech/cargo-unleash)
- [release-plz](https://crates.io/crates/release-plz)
- [cargo-workspaces](https://crates.io/crates/cargo-workspaces)

## Semver Compatibility

cargo-release's versioning tracks compatibility for the binaries, not the API.  We upload to
crates.io to distribute the binary.  If using this as a library, be sure to pin the version
with a `=` version requirement operator.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
  at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
