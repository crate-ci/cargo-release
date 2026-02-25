# cargo release

> Streamline your crate release process

`cargo release` extends `cargo publish` with common release practices like
validation, version management, tagging, and pushing.
The process is customizable but with defaults that should help you get up and running quickly.

Features include:
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
  - [Updating changelogs](docs/faq.md#maintaining-changelog)
  - Update tags in `Dockerfile`s
- Pre-release hook for extra customization, including
  - [CHANGELOG generation](docs/faq.md#maintaining-changelog)

## Install

Current release: 1.1.1

```sh
cargo install cargo-release
```

## Usage

```console
$ cargo release [level]
<dry-run output>
$ cargo release [level] --execute
```

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

*Note:* the packaging and verifying steps will list the current version and not
the bumped version because `cargo release` delegates to `cargo publish` which
does not know about the in-memory-only version bump ([#890](https://github.com/crate-ci/cargo-release/issues/890)).

## Related tools

- [release-pr Action](https://github.com/cargo-bins/release-pr)
- [cargo-smart-release](https://github.com/Byron/cargo-smart-release)
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

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/license/mit>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.
