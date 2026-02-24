# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/)
and this project adheres to [Semantic Versioning](https://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

### Fixes

- Attempt to fix pre-built binaries

## [1.1.0] - 2026-02-23

### Features

- Support TOML 1.1
- Support `{{repository}}` variable in `pre-release-replacements`

### Performance

- Remove an unused build dependency

## [1.0.0] - 2026-02-11

## [0.25.22] - 2025-11-13

### Fixes

- Don't require nightly for the stabilized Cargo-native workspace publishing

## [0.25.21] - 2025-11-13

### Compatibility

- Build and runtime MSRV is now 1.90

### Features

- Stabilize Cargo-native workspace publishing

## [0.25.20] - 2025-10-06

### Fixes

- Include registry name in "already published" error

## [0.25.19] - 2025-09-26

### Fixes

- Ensure `--unpublished` can select packages without any other packages selected

## [0.25.18] - 2025-04-09

### Internal

- Dependency updates

## [0.25.17] - 2025-02-05

### Fixes

- Upgrade "unrendered variable" debug message to warning

## [0.25.16] - 2025-01-31

### Fixes

- Use workspace default commit message (rather than package) when workspace inheritance forces consolidated-commits on

## [0.25.15] - 2024-12-31

### Fixes

- *(unstable)* Don't try to workspace-publish packages that have `publish = false`

## [0.25.14] - 2024-12-12

### Features

- Support for cargo's nightly workspace publishing (requires `cargo +nightly release -Zworkspace-publish ...`)

## [0.25.13] - 2024-11-15

### Compatibility

- Build MSRV is now 1.80

### Fixes

- Improve `allow-branch` error

## [0.25.12] - 2024-10-07

### Compatibility

- Build MSRV is now 1.79

### Fixes

- Report current dir for hook error messages

## [0.25.11] - 2024-09-06

### Compatibility

- Build MSRV is now 1.78
- Runtime MSRV is now 1.66

### Fixes

- Removed our own wait-for-publish logic, relying on Cargo instead

## [0.25.10] - 2024-06-21

### Features

- Add `--certs-source` for overriding where certs are read from

## [0.25.9] - 2024-06-21

### Features

- Add `rate-limit.new-packages`, `rate-limit.existing-packages`  workspace config to override the defaults

## [0.25.8] - 2024-05-29

### Internal

- Update dependencies

## [0.25.7] - 2024-04-25

### Compatibility

- MSRV is now 1.76

### Fixes

- Improve error message on hook failure due to permissions

## [0.25.6] - 2024-02-27

### Fixes

- Improve diff output

## [0.25.5] - 2024-02-08

### Fixes

- Ensure needed dependency features are enabled

## [0.25.4] - 2024-01-17

### Features

- Add `--prev-tag-name` to `cargo release changes`

## [0.25.3] - 2024-01-10

### Features

- Automatically skip releasing Cargo 1.75's version-less manifests

## [0.25.2] - 2024-01-09

### Fixes

- Wait for crates.io publishes, not for other registries

## [0.25.1] - 2023-12-27

### Fixes

- Don't talk to crates.io when publishing to an alternative registry

## [0.25.0] - 2023-11-01

### Fixes

- Always use sparse index

## [0.24.12] - 2023-09-11

### Compatibility

- Update MSRV to 1.70.0

### Features

- Style help text like cargo nightly

## [0.24.11] - 2023-06-26

## [0.24.10] - 2023-04-17

### Fixes

- Allow disabling of vendored libgit2/openssl

## [0.24.9] - 2023-04-15

### Fixes

- Don't corrupt TOML table headers with newlines

## [0.24.8] - 2023-03-17

### Fixes

- Report the config file we failed to parse

## [0.24.7] - 2023-03-17

### Fixes

- Vendor OpenSSL to avoid problems from the local system

## [0.24.6] - 2023-03-13

### Fixes

- When editing TOML, don't lose standard tables under dotted tables

## [0.24.5] - 2023-02-28

### Internal

- Dependency update

## [0.24.4] - 2023-01-12

### Fixes

- `cargo release owner` will only prompt for crates that will be checked

## [0.24.3] - 2023-01-06

### Features

- New `metadata` config field for controlling how the version build metadata is updated (or not)

## [0.24.2] - 2023-01-03

### Fixes

- Support `package.publish` being a list of registries

## [0.24.1] - 2022-12-13

### Fixes

- Don't error out if nothing to commit

## [0.24.0] - 2022-11-28

### Breaking Changes

- Remove support for unused config fields (post-release)

### Fixes

- Help catch unused template variables by logging them

## [0.23.1] - 2022-11-16

### Fixes

- Respect `--unpublished`

## [0.23.0] - 2022-11-05

### Breaking Changes

- No longer assume `package.publish = false` also means that releases are disabled (#597)

## [0.22.4] - 2022-11-05

### Fixes

- Skip remote check if not pushing

## [0.22.3] - 2022-11-05

### Fixes

- Ensure pushes are atomic

## [0.22.2] - 2022-10-31

### Fixes

- Limit rate-limiting to crates being published

## [0.22.1] - 2022-10-21

### Fixes

- Don't claim the user excluded a crate when the config did
- Actually push the branch
- Show we'll push the branch in dry-run mode, even if we won't as its better than not showing it even if we will

## [0.22.0] - 2022-10-21

### Highlights

The goal of this release is improved workspace support, including
- [Workspace inheritance support](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-package-table)
- Greater control over versioning by supporting calling `cargo release version` as needed and then `cargo release --unpublished`
- Setting configured crate owners when publishing new crates
- Identifying failures early like missing metadata, hitting rate limits, etc
- Inspect changes with `cargo release changes`, including conventional commit support
- Improved defaults

This does mean that `cargo release` (no other args) changed from recovering
from a failed release to releasing the currently specified versions of crates.
Recovery is now done more manually with `cargo release publish`, etc.

### Breaking Changes

- Removed `dev-version` support
- `consolidate-commits` is now the default for workspaces
  - It is also now all-or-nothing
- `consolidate-pushes` is now exclusively used
- `dependent-versions = "upgrade"` is now the default
  - Removed `ignore`, `warn`, and `error`
- `Cargo.toml`'s `package.publish = false` disables release
- Removed `--dump-config` in favor of `cargo release config`
- Remove `--token` in favor in favor of more secure ways of authenticating
- `cargo release` is no longer used for recovery, instead use `cargo release publish`, `cargo release tag`, etc
- Error if nothing to release
- Changed standard exit code to 101

### Compatibility

MSRV is now 1.64.0

### Fixes

- Turn some verification errors into warnings on steps
- Run replacements when no version is bumped
- Be smarter about finding previous tags
- Bail out early when we'll hit crates.io rate limits
- Bail out early when we'll hit `cargo publish` missing field errors
- Implicitly layer package over workspace for workspace config when not in a workspace
- Only update versions for path dependencies
- Cleaned up output
- Ignore tests when tracking changes

### Features

- `package.version.workspace = true` support
  - Forces `consolidate-commits = true`
  - Forces `shared-version = "workspace"`
- `dependency.<name>.workspace = true` support
- `package.publish.workspace = true` support
- `owners = []` to set crate owners for new workspace members
  - Use `cargo release owner` to update owners for existing crates
- In addition to `shared-version = true`, we now support named groups, like `shared-version = "foo"`
- `--unpublished` flag to automatically release unpublished crates
- Expose `changes`, `hook`, and `commit` steps

## [0.21.4] - 2022-10-14

### Fixes

- Fail again when pre-release checks fail

## [0.21.3] - 2022-10-13

### Features

- Expose release steps as subcommands, useful for
  - Custom release processes
  - Verify configuration / cargo-release behavior
  - Recovering on failure

## [0.21.2] - 2022-09-28

### Fixes

- Polish help output

## [0.21.1] - 2022-07-12

### Fixes

- Load workspace config from the actual workspace manifest

## [0.21.0] - 2022-05-26

### Breaking Change

- Template substitutions are now performed on pre-release-hook arguments

### Features

- Template substitutions are now performed on pre-release-hook arguments

## [0.20.6] - 2022-05-26

### Fixes

- Don't accidentally publish a `default-member` instead of the root crate

## [0.20.5] - 2022-04-13

### Features

- Add `--allow-branch`

## [0.20.4] - 2022-04-12

### Fixes

- Be explicit on dry-run failures

## [0.20.3] - 2022-03-07

### Fixes

- Don't fail dirty detection when a `HEAD` file exists
- Show more details on dry-run

## [0.20.2] - 2022-02-15

### Fixes

- Break cycles at dev dependencies

## [0.20.1] - 2022-02-04

### Fixes

- Add missing `--metadata` to complement `-m`

## [0.20.0] - 2022-02-02

### Breaking Changes

- **Replacements:** Changed `^` / `$` to match start/end of lines rather than file

### Fixes

- **Replacements:** Changed `^` / `$` to match start/end of lines rather than file

## [0.19.4] - 2022-01-25

### Features

- Lightweight tag support by setting `message = ""`

## [0.19.3] - 2022-01-21

### Fixes

- Preserve dependent version requirement format

## [0.19.2] - 2022-01-17

### Features

- `--target` flag to control what target is used for the verification build during publish

## [0.19.1] - 2022-01-12

### Fixes

- Don't panic on `release=false`

## [0.19.0] - 2022-01-07

### Breaking Changes

- Dirty repo check will now check the entire workspace

Config
- `sign-commit` will no longer sign tags, instead set `sign-tag`
- Removed `disable-release` in favor of `release`
- Removed `disable-publish` in favor of `publish`
- Removed `no-verify` in favor of `verify`
- Removed `disable-push` in favor of `push`
- Removed `no-dev-version` in favor of `dev-version`
- Removed `disable-tag` in favor of `tag`

Args:
- Removed `--skip-publish` in favor of `--no-publish`
- Removed `--skip-push` in favor of `--no-push`
- Removed `--skip-tag` in favor of `--no-tag`

Template
- `{{version}}`, `{{prev_version}}`, and `{{next_version}}` now exclude the build field which is exposed in `{{metadata}}` etc

Hook
- `${NEXT_VERSION}` and `${PREV_VERSION}` now exclude the build field which is exposed in `${METADATA}` etc

### Features

- Automatically share tags between crates by giving them the same name
  - Recommended to use with `shared-version = true`

### Fixed

- With `shared-version`, bump to highest shared version, rather than error on mismatch
- `cargo-release release` will now skip publishing crates that are already published
- Report tag name conflicts earlier in the process
- `--dump-config` will now also include defaults
- Don't fail on dry-run release in a workspace (from intra-workspace dependency updates)
- `cargo-release <version>` will now work with `-m <build>`
- `cargo-release <version>` will now carry over `-m <build>` from a prior run if none is specified
- Make dirty check more expansive so we don't accidentally commit unexpected filed

## [0.18.8] - 2021-12-31

### Features

- `--dump-config` flag to see defaults and debug config layering

## [0.18.7] - 2021-12-27

### Fixed

- Don't error out when excluding workspace packages that would be downgraded by the current run

## [0.18.6] - 2021-12-08

### Fixed

- Consider optional dependencies when evaluating release order

## [0.18.5] - 2021-11-16

### Features

- Warn users when skipping crates in a workspace that have no changes

### Fixed

- Use static crt on Windows
- Tweak log levels to avoid needing to show all traces to see whats happening

## [0.18.4] - 2021-10-30

### Fixed

- Vendor libgit2 for a more consistent experience

## [0.18.3] - 2021-10-26

### Fixed

- Fix bug where we ignored `push` config

## [0.18.2] - 2021-10-11

### Fixed

- Crash when setting `publish = false` in `Cargo.toml` **and**` in a config file / commandline

## [0.18.1] - 2021-10-09

### Fixed

- Introduced `cargo publish` fix for workspaces that was meant to be in before 0.18

## [0.18.0] - 2021-10-08

### Breaking Changes

`dev-version` is now disabled by default.  This to encourage people to not use
it as it makes it harder for dependent crates to `[patch]` in a version from
git.

### Features

- Opt-in shared crate version.  For now, it just errors on mismatch.  This lets you reference the version in consolidated commits.

### Fixed

- In dev-version commit, render `{{next_version}}`
- Disable always-sleep after publish.  We believe the underlying problem
  preventing us from detecting the crate from being released is fixed.  If you
  run into problems, you can set the env variable `PUBLISH_GRACE_SLEEP`.
- Let packages override consolidated actions
- Switched command-line to more common `no-` prefixes
- Added positive and negative version of each command-line flag to allow overriding the config, and not just defaults.
- Added positive versions of each negative (`disable_`, `no_`) config field
- **Deprecated** all negative (`disable_`, `no_`) config fields

## [0.17.1] - 2021-08-24

### Fixed

- Correctly detect changes for crates outside of the root

## [0.17.0] - 2021-08-23

### Features

- Support `~/.config/cargo-release/release.toml`
- Run `cargo publish` during dry-runs to help catch publish-specific errors
- Add `allow-branch` config setting to limit what branches a release can happen from
- Support `Cargo.toml`s `workspace.metadata`

### Fixed

- In theory, finally fixed it so we properly wait between publishing of crates in a workspace
- Don't warn a user about releasing a crate without changes if a dependency changed
- Notify for all `[[bin]]` crates on `Cargo.lock` change, rather than just the root crate
- Made clearer what are fatal errors during dry-run (since dry-run doesn't stop for them)
- Gracefully handle path-only dependencies which are especially important for cycles.
- Correctly update dependents on post-release version bump.
- Log what was dirty about a repo to make it easier for people to report problems
- Allow pushing even when there isn't a tracking branch
- Specifying `--package` should switch us to opt-in

### Breaking Changes

- `--dry-run` is now the default.   Pass `--execute` to perform the release.
- `exclude-paths` config setting was removed; we now rely on `cargo package --list` to know which files to check for changes.

## [0.16.3] - 2021-08-01

## [0.16.2] - 2021-07-15

### Fixed

* Respect `disable_push` flag at package level.

## [0.16.1] - 2021-07-04

### Fixed

* Submodule operation dir issue

## [0.16.0] - 2021-07-03

### Added

* Git dirty check for submodules

### Changed

* Prior sharing of pushes between workspace crates is now behind the flag `consolidate-pushes`

### Fixed

* Avoid panic on invalid Cargo.toml entry

## [0.15.1] - 2021-06-24

### Fixed

* Fixed issue where the versions of cfg specific dependencies wouldn't be properly bumped.

## [0.15.0] - 2021-06-19

### Added

* `push-options` to send flags to the server, on push.  Current limitations include:
  * Only on branch and not tag push
  * Operates at the workspace level
  * No placeholders are supported

### Changed

* `disable-push`, `push-remote` now only apply at the workspace level, when in a workspace.
* tags are pushed before branch as requested in #250

## [0.14.0] - 2021-06-16

### Added

* Add `PUBLISH_GRACE_SLEEP` environment variable that allows to set the number of seconds to sleep between
  two invocations of `cargo publish`. Default is `5`
* Do not sleep between publishes on dry runs

### Changed

* New `disable-release` config flag to skip crates in a workspace
* Warn on detached HEADs and being behind the remote
  * **Note:** This means we are now doing a `git fetch` at the beginning, even with `--dry-run`

## [0.13.11] - 2021-03-25

## [0.13.10] - 2020-12-28

### Changed

* Dependencies updated
* Add sleep between publish [#247]

## [0.13.9] - 2020-11-29

### Changed

* Improved diff view in dry-run mode

## [0.13.8] - 2020-09-29

### Added

* New option `post-release-replacements` support [#228]

## [0.13.7] - 2020-09-27

### Changed

* Upgraded crate-index and minimum rust version [#227]

## [0.13.6] - 2020-08-31

### Fixed

* `min`/`max` bug with replacement settings [#225]
* Better error message for IOError [#226]

## [0.13.5] - 2020-07-04

### Added

* Added new option `sign-tag` for tag signing only

## [0.13.4] - 2020-05-10

### Changed

* Changed default timeout on waiting crate to land on crates.io [#207]
* Changed change detection log to debug [#208]

## [0.13.3] - 2020-03-13

### Added

* Ability to upload to alternate registries (though wait-for-publish
  is skipped) [#203]

### Fixed

* Prerelease check for replacement

## [0.13.1] - 2020-03-01

### Added

* Config: `exclude_paths` list of globs to get more accurate listing of files-changed [#149]
* CLI: `--token` can be used to specify the token used by `cargo publish`

### Fixed

* Take 2 on waiting for a crate to be published before publishing the next one [#194]

## [0.13.0] - 2019-12-09

### Added

* Notify users on unchanged crates when releasing workspace [#148]
* Strict check on replacements [#187]
* Trace replacement diff on dry-run [#171]
* Allow workspace release commits to be consolidated [#181]
* Releasing specific version [#191]
* `tag_name` is now available in replacements and can be useful for
  changelog generation in multi-crate workspace [#168]

### Changed

* Renamed option "pro-release-commit-message" to
  "post-release-commit-message" [#140]
* Use logging for output [#152]
* Also check untracked files in initial dirty check [#146]
* `[package.metadata.release]` in `$CRATE/Cargo.toml` now has a higher
  priority than `$CRATE/release.toml` [7cc9890] [#181]
* Confirmation is prompted for even when there is no version bump
  [47bf645] [#175]

### Fixed

* Fixed issue when crate.io didn't update in time that causing
  workspace release failed [#183]

### Removed

* Doc upload removed as the community has moved to [docs.rs](https://docs.rs) [#176]

## [0.12.4] - 2019-08-03

### Changed

* Fixed commit message after release #136

## [0.12.3] - 2019-07-28

### Changed

* Only update dependents when needed #135

## [0.12.2] - 2019-07-24

### Changed

* Fixed issue when updating dependency version in workspace #130

## [0.12.1] - 2019-07-18

### Changed

* Fixed serde version as 1.0.95 was yanked

## [0.12.0] - 2019-07-17

### Added

* Workspace support #66
* Layered config support #111
* Support for tag name customization #125

### Changed

* Using `v` as default version tag prefix #127
* Improved cargo binary detection #88 #89
* Doc update

<!-- next-url -->
[Unreleased]: https://github.com/crate-ci/cargo-release/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/crate-ci/cargo-release/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/crate-ci/cargo-release/compare/v0.25.22...v1.0.0
[0.25.22]: https://github.com/crate-ci/cargo-release/compare/v0.25.21...v0.25.22
[0.25.21]: https://github.com/crate-ci/cargo-release/compare/v0.25.20...v0.25.21
[0.25.20]: https://github.com/crate-ci/cargo-release/compare/v0.25.19...v0.25.20
[0.25.19]: https://github.com/crate-ci/cargo-release/compare/v0.25.18...v0.25.19
[0.25.18]: https://github.com/crate-ci/cargo-release/compare/v0.25.17...v0.25.18
[0.25.17]: https://github.com/crate-ci/cargo-release/compare/v0.25.16...v0.25.17
[0.25.16]: https://github.com/crate-ci/cargo-release/compare/v0.25.15...v0.25.16
[0.25.15]: https://github.com/crate-ci/cargo-release/compare/v0.25.14...v0.25.15
[0.25.14]: https://github.com/crate-ci/cargo-release/compare/v0.25.13...v0.25.14
[0.25.13]: https://github.com/crate-ci/cargo-release/compare/v0.25.12...v0.25.13
[0.25.12]: https://github.com/crate-ci/cargo-release/compare/v0.25.11...v0.25.12
[0.25.11]: https://github.com/crate-ci/cargo-release/compare/v0.25.10...v0.25.11
[0.25.10]: https://github.com/crate-ci/cargo-release/compare/v0.25.9...v0.25.10
[0.25.9]: https://github.com/crate-ci/cargo-release/compare/v0.25.8...v0.25.9
[0.25.8]: https://github.com/crate-ci/cargo-release/compare/v0.25.7...v0.25.8
[0.25.7]: https://github.com/crate-ci/cargo-release/compare/v0.25.6...v0.25.7
[0.25.6]: https://github.com/crate-ci/cargo-release/compare/v0.25.5...v0.25.6
[0.25.5]: https://github.com/crate-ci/cargo-release/compare/v0.25.4...v0.25.5
[0.25.4]: https://github.com/crate-ci/cargo-release/compare/v0.25.3...v0.25.4
[0.25.3]: https://github.com/crate-ci/cargo-release/compare/v0.25.2...v0.25.3
[0.25.2]: https://github.com/crate-ci/cargo-release/compare/v0.25.1...v0.25.2
[0.25.1]: https://github.com/crate-ci/cargo-release/compare/v0.25.0...v0.25.1
[0.25.0]: https://github.com/crate-ci/cargo-release/compare/v0.24.12...v0.25.0
[0.24.12]: https://github.com/crate-ci/cargo-release/compare/v0.24.11...v0.24.12
[0.24.11]: https://github.com/crate-ci/cargo-release/compare/v0.24.10...v0.24.11
[0.24.10]: https://github.com/crate-ci/cargo-release/compare/v0.24.9...v0.24.10
[0.24.9]: https://github.com/crate-ci/cargo-release/compare/v0.24.8...v0.24.9
[0.24.8]: https://github.com/crate-ci/cargo-release/compare/v0.24.7...v0.24.8
[0.24.7]: https://github.com/crate-ci/cargo-release/compare/v0.24.6...v0.24.7
[0.24.6]: https://github.com/crate-ci/cargo-release/compare/v0.24.5...v0.24.6
[0.24.5]: https://github.com/crate-ci/cargo-release/compare/v0.24.4...v0.24.5
[0.24.4]: https://github.com/crate-ci/cargo-release/compare/v0.24.3...v0.24.4
[0.24.3]: https://github.com/crate-ci/cargo-release/compare/v0.24.2...v0.24.3
[0.24.2]: https://github.com/crate-ci/cargo-release/compare/v0.24.1...v0.24.2
[0.24.1]: https://github.com/crate-ci/cargo-release/compare/v0.24.0...v0.24.1
[0.24.0]: https://github.com/crate-ci/cargo-release/compare/v0.23.1...v0.24.0
[0.23.1]: https://github.com/crate-ci/cargo-release/compare/v0.23.0...v0.23.1
[0.23.0]: https://github.com/crate-ci/cargo-release/compare/v0.22.4...v0.23.0
[0.22.4]: https://github.com/crate-ci/cargo-release/compare/v0.22.3...v0.22.4
[0.22.3]: https://github.com/crate-ci/cargo-release/compare/v0.22.2...v0.22.3
[0.22.2]: https://github.com/crate-ci/cargo-release/compare/v0.22.1...v0.22.2
[0.22.1]: https://github.com/crate-ci/cargo-release/compare/v0.22.0...v0.22.1
[0.22.0]: https://github.com/crate-ci/cargo-release/compare/v0.21.4...v0.22.0
[0.21.4]: https://github.com/crate-ci/cargo-release/compare/v0.21.3...v0.21.4
[0.21.3]: https://github.com/crate-ci/cargo-release/compare/v0.21.2...v0.21.3
[0.21.2]: https://github.com/crate-ci/cargo-release/compare/v0.21.1...v0.21.2
[0.21.1]: https://github.com/crate-ci/cargo-release/compare/v0.21.0...v0.21.1
[0.21.0]: https://github.com/crate-ci/cargo-release/compare/v0.20.6...v0.21.0
[0.20.6]: https://github.com/crate-ci/cargo-release/compare/v0.20.5...v0.20.6
[0.20.5]: https://github.com/crate-ci/cargo-release/compare/v0.20.4...v0.20.5
[0.20.4]: https://github.com/crate-ci/cargo-release/compare/v0.20.3...v0.20.4
[0.20.3]: https://github.com/crate-ci/cargo-release/compare/v0.20.2...v0.20.3
[0.20.2]: https://github.com/crate-ci/cargo-release/compare/v0.20.1...v0.20.2
[0.20.1]: https://github.com/crate-ci/cargo-release/compare/v0.20.0...v0.20.1
[0.20.0]: https://github.com/crate-ci/cargo-release/compare/v0.19.4...v0.20.0
[0.19.4]: https://github.com/crate-ci/cargo-release/compare/v0.19.3...v0.19.4
[0.19.3]: https://github.com/crate-ci/cargo-release/compare/v0.19.2...v0.19.3
[0.19.2]: https://github.com/crate-ci/cargo-release/compare/v0.19.1...v0.19.2
[0.19.1]: https://github.com/crate-ci/cargo-release/compare/v0.19.0...v0.19.1
[0.19.0]: https://github.com/crate-ci/cargo-release/compare/v0.18.8...v0.19.0
[0.18.8]: https://github.com/crate-ci/cargo-release/compare/v0.18.7...v0.18.8
[0.18.7]: https://github.com/crate-ci/cargo-release/compare/v0.18.6...v0.18.7
[0.18.6]: https://github.com/crate-ci/cargo-release/compare/v0.18.5...v0.18.6
[0.18.5]: https://github.com/crate-ci/cargo-release/compare/v0.18.4...v0.18.5
[0.18.4]: https://github.com/crate-ci/cargo-release/compare/v0.18.3...v0.18.4
[0.18.3]: https://github.com/crate-ci/cargo-release/compare/v0.18.2...v0.18.3
[0.18.2]: https://github.com/crate-ci/cargo-release/compare/v0.18.1...v0.18.2
[0.18.1]: https://github.com/crate-ci/cargo-release/compare/v0.18.0...v0.18.1
[0.18.0]: https://github.com/crate-ci/cargo-release/compare/v0.17.1...v0.18.0
[0.17.1]: https://github.com/crate-ci/cargo-release/compare/v0.17.0...v0.17.1
[0.17.0]: https://github.com/crate-ci/cargo-release/compare/v0.16.6...v0.17.0
[0.16.6]: https://github.com/crate-ci/cargo-release/compare/v0.16.5...v0.16.6
[0.16.5]: https://github.com/crate-ci/cargo-release/compare/v0.16.4...v0.16.5
[0.16.4]: https://github.com/crate-ci/cargo-release/compare/v0.16.3...v0.16.4
