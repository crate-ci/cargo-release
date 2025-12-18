# `cargo-release` Reference

## CLI Arguments

```console
$ cargo-release release -h
Cargo subcommand for you to smooth your release process.

Usage: cargo release [OPTIONS] [LEVEL|VERSION]
       cargo release <STEP>

Steps:
  changes  Print commits since last tag
  version  Bump crate versions
  replace  Perform pre-release replacements
  hook     Run pre-release hooks
  commit   Commit the specified packages
  publish  Publish the specified packages
  owner    Ensure owners are set on specified packages
  tag      Tag the released commits
  push     Push tags/commits to remote
  config   Dump workspace configuration
  help     Print this message or the help of the given subcommand(s)

Arguments:
  [LEVEL|VERSION]  Either bump by LEVEL or set the VERSION for all selected packages [possible
                   values: major, minor, patch, release, rc, beta, alpha]

Options:
      --manifest-path <PATH>         Path to Cargo.toml
  -p, --package <SPEC>               Package to process (see `cargo help pkgid`)
      --workspace                    Process all packages in the workspace
      --exclude <SPEC>               Exclude packages from being processed
      --unpublished                  Process all packages whose current version is unpublished
  -m, --metadata <METADATA>          Semver metadata
  -x, --execute                      Actually perform a release. Dry-run mode is the default
      --no-confirm                   Skip release confirmation and version preview
      --prev-tag-name <NAME>         The name of tag for the previous release
  -c, --config <PATH>                Custom config file
      --isolated                     Ignore implicit configuration files
  -Z <FEATURE>                       Unstable options
      --sign                         Sign both git commit and tag
      --dependent-version <ACTION>   Specify how workspace dependencies on this crate should be
                                     handed [possible values: upgrade, fix]
      --allow-branch <GLOB[,...]>    Comma-separated globs of branch names a release can happen from
      --certs-source <CERTS_SOURCE>  Indicate what certificate store to use for web requests
                                     [possible values: webpki, native]
  -q, --quiet...                     Pass many times for less log output
  -v, --verbose...                   Pass many times for more log output
  -h, --help                         Print help (see more with '--help')
  -V, --version                      Print version

Commit:
      --sign-commit  Sign git commit

Publish:
      --no-publish           Do not run cargo publish on release
      --registry <NAME>      Cargo registry to upload to
      --no-verify            Don't verify the contents by building them
      --features <FEATURES>  Provide a set of features that need to be enabled
      --all-features         Enable all features via `all-features`. Overrides `features`
      --target <TRIPLE>      Build for the target triple

Tag:
      --no-tag               Do not create git tag
      --sign-tag             Sign git tag
      --tag-prefix <PREFIX>  Prefix of git tag, note that this will override default prefix based on
                             sub-directory
      --tag-name <NAME>      The name of the git tag

Push:
      --no-push             Do not run git push in the last step
      --push-remote <NAME>  Git remote to push

```

### Bump level

* `release` (default): Remove the pre-release extension; if any (0.1.0-alpha.1 -> 0.1.0, 0.1.0 -> 0.1.0).
* `patch`:
  * If version has a pre-release, then the pre-release extension is removed (0.1.0-alpha.1 -> 0.1.0).
  * Otherwise, bump the patch field (0.1.0 -> 0.1.1)
* `minor`: Bump minor version (0.1.0-pre -> 0.2.0)
* `major`: Bump major version (0.1.0-pre -> 1.0.0)
* `alpha`, `beta`, and `rc`: Add/increment pre-release to your version
  (1.0.0 -> 1.0.1-rc.1, 1.0.1-alpha -> 1.0.1-rc.1, 1.0.1-rc.1 ->
  1.0.1-rc.2)
* *[version]*: bump version to given version. The version has to
  be a valid semver string and greater than current version as in
  semver spec.

## Configuration

### Sources

Package configuration is read from the following (in precedence order)
- Command line arguments
- File specified via `--config PATH`
- `$CRATE/Cargo.toml` (`[package.metadata.release]` table)
- `$CRATE/release.toml`
- `$WORKSPACE/Cargo.toml` (`[workspace.metadata.release]` table)
- `$WORKSPACE/release.toml`
- *platform dependent*
  - linux: `$HOME/.config/cargo-release/release.toml`
  - windows: `%FOLDERID_RoamingAppData%/cargo-release/release.toml`
  - mac: `$HOME/Library/Application Support/cargo-release/release.toml`
- `$HOME/.release.toml`

Workspace configuration is read from the following (in precedence order)
- Command line arguments
- File specified via `--config PATH`
- `$WORKSPACE/Cargo.toml` (`[workspace.metadata.release]` table)
- `$WORKSPACE/release.toml`
- `$HOME/.config/cargo-release/release.toml`
- `$HOME/.release.toml`

### Format

Summary of configuration (see below for details)
```toml
allow-branch = ["*", "!HEAD"]
sign-commit = false
sign-tag = false
registry = "..."
release = true
push = true
push-remote = "origin"
push-options = ""
shared-version = false
consolidate-commits = true
pre-release-commit-message = "chore: Release"
tag = true
tag-message = "chore: Release"
tag-prefix = "..."
tag-name = "{{prefix}}v{{version}}"
pre-release-replacements = []
pre-release-hook = ["..."]
publish = true
verify = true
owners = []
enable-features = []
enable-all-features = false
target = "..."
dependent-version = "upgrade"
metadata = "optional"
rate-limit.new-packages = 5
rate-limit.existing-packages = 30
certs-source = "webpki"
```

### Configuration keys

Note: fields are from the [package-configuration](#sources) unless otherwise specified.

### `allow-branch`

[**Workspace Configuration**](#source)

- Type: list of globs
- Default: `["*", "!HEAD"]`
- CLI: `--allow-branch`

Which branches are allowed to be released from

### `sign-commit`

- Type: bool
- Default: `false`
- CLI: `--sign-commit`

Use GPG to sign git commits generated by cargo-release.
[Further information](https://git-scm.com/book/en/v2/Git-Tools-Signing-Your-Work).
In 0.14 `sign-commit` is to control signing for commit only, use `sign-tag` for tag signing.

### `sign-tag`

- Type: bool
- Default: `false`
- CLI: `--sign-tag`

Use GPG to sign git tag generated by cargo-release.

### `registry`

- Type: string
- CLI: `--registry`

Cargo registry name to publish to (default uses Rust's default, which goes to `crates.io`)

### `release`

- Type: bool
- Default: `true`
- CLI: `--package`

Release this crate (usually disabled for internal crates in a workspace)

### `push`

- Type: bool
- Default: `true`
- CLI: `--no-push`

Git push the branch / tags

### `push-remote`

- Type: string
- Default: `"origin"`
- CLI: `--push-remote`

Default git remote to push

### `push-options`

- Type: list of strings

Flags to send to the server when doing a `git push`

### `shared-version`

- Type: bool or string
- Default: `false`

Ensure all crates with `shared-version` are the same version.
May also be a string to create named subsets of shared versions

### `consolidate-commits`

- Type: bool
- Default: `true`

When releasing a workspace, use a single commit for the pre-release version bump.
Commit settings will be read from the workspace-config.

### `pre-release-commit-message`

- Type: string
- Default: `"chore: Release"`

A commit message template for release.

See also [Placeholders(#placeholders)

### `tag`

- Type: bool
- Default: true
- CLI: `--no-tag`

Create git tag for the version

### `tag-message`

- Type: string
- Default: `"chore: Release"`

A message template for an annotated tag (set to blank for lightweight tags). The placeholder `{{tag_name}}` and `{{prefix}}` (the tag prefix) is supported in addition to the global placeholders mentioned below.

See also [Placeholders(#placeholders)

### `tag-prefix`

- Type: string
- CLI: `--tag-prefix`

Prefix of git tag, note that this will override default prefix based on crate name.

See also [Placeholders(#placeholders)

### `tag-name`

- Type: string
- Default: `"{{prefix}}v{{version}}"`
- CLI: `--tag-name`

The name of the git tag.
The placeholder `{{prefix}}` (the tag prefix) is supported in addition to the global placeholders mentioned below.

See also [Placeholders(#placeholders)

### `pre-release-replacements`

- Type: array of tables (see below)
- Default: `[]`

Specify files that cargo-release will search and replace with new version for the release commit

This field is an array of tables with the following

* `file`: the file to search and replace
* `search`: [regex](https://docs.rs/regex/latest/regex/) that matches string you want to replace
* `replace`: the replacement string; you can use the any of the placeholders
  mentioned below. Regex patterns, such as `$1`, are also valid for referring to
  captured groups.
* `min` (default is `1`): Minimum occurrences of `search`.
* `max` (optional): Maximum occurrences of `search`.
* `exactly` (optional): Number of occurrences of `search`.
* `prerelease` (default is `false`): Run the replacement when bumping to a pre-release level.

See [Cargo.toml](https://github.com/crate-ci/cargo-release/blob/master/Cargo.toml) for example.

### `pre-release-hook`

- Type: list of arguments

Provide a command to run before `cargo-release` commits version change. If the return code of hook command is greater than 0, the release process will be aborted.

The following environment variables are made available to `pre-release-hook`:

* `PREV_VERSION`: The version before `cargo-release` was executed (before any version bump).
* `PREV_METADATA`: The version's metadata field before `cargo-release` was executed (before any version bump).
* `NEW_VERSION`: The current (bumped) crate version.
* `NEW_METADATA`: The current (bumped) crate version's metadata field.
* `DRY_RUN`: Whether the release is actually happening (`true` / `false`)
* `CRATE_NAME`: The name of the crate.
* `WORKSPACE_ROOT`: The path to the workspace.
* `CRATE_ROOT`: The path to the crate.

See also [Placeholders(#placeholders)

### `publish`

- Type: bool
- Default: `true`
- CLI: `--no-publish`

`cargo publish` right now, see [manifest `publish` field](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish--field-optional) to permanently disable publish.
See `release` for disabling the complete release process.

### `verify`

- Type: bool
- Default: `true`
- CLI: `--no-verify`

Verify the contents by building them

### `owners`

- Type: list of logins
- Default: `[]`
- CLI:

Ensure these logins are marked as owners

### `enable-features`

- Type: list of names
- Default: `[]`
- CLI: `--features`

Provide a set of feature flags that should be passed to `cargo publish` (requires rust 1.33+)

### `enable-all-features`

- Type: bool
- Default: `false`
- CLI: `--all-features`

Signal to `cargo publish`, that all features should be used (requires rust 1.33+)

### `target`

- Type: string

Target triple to use for the verification build

### `dependent-version`

- Type: `upgrade`, `fix`, `error`, `warn`, `ignore`
- Default: `"upgrade"`

Policy for upgrading path dependency versions within the workspace

### `metadata`

- Type: `optional`, `required`, `ignore`, `persistent`
- Default: `"optional"`

Policy for presence of absence of `--metadata` flag when changing the version

### `rate-limit.new-packages`

- Type: integer
- Default: `5`

Rate limit for publishing new packages

### `rate-limit.existing-packages`

- Type: integer
- Default: `30`

Rate limit for publishing existing packages

### `certs-source`

- Type: `webpki`, `native`
- Default: `"webpki"`

Policy for using Mozilla's standard certificate root of trust (`webpki`) or using the system certificate root of trust (`native`)

### Placeholders

The following fields support placeholders for information about your release:

- `pre-release-commit-message`
- `tag-message`
- `tag-prefix`
- `tag-name`
- `pre-release-hook`

The following placeholders are supported:

* `{{prev_version}}`: The version before `cargo-release` was executed (before any version bump).
* `{{prev_metadata}}`: The version's metadata before `cargo-release` was executed (before any version bump).
* `{{version}}`: The current (bumped) crate version.
  * Only works for `pre-release-commit-message` when `consolidate-commits = false` or when using `shared-version = true`.
* `{{metadata}}`: The current (bumped) crate version's metadata field.
* `{{crate_name}}`: The name of the current crate in `Cargo.toml`.
* `{{date}}`: The current date in `%Y-%m-%d` format.
* `{{prefix}}` (only valid for `tag-name` / `tag-message`): The value prepended to the tag name.
* `{{tag_name}}` (only valid for `tag-message`): The name of the git tag.

## Environment variables

* `PUBLISH_GRACE_SLEEP`: sleep timeout between crates publish when releasing from workspace. This is a workaround to make previous crate discoverable on crates.io.
