# FAQ

## How do I update my README or other files

Cargo-release 0.8 allows you to search and replace version string in
any project or source file.

For example, to update the version in predicates
[`README.md`](https://github.com/assert-rs/predicates-rs/blob/master/README.md):
```toml
[dependencies]
predicates = "1.0.2"
```

You use the following
[`release.toml`](https://github.com/assert-rs/predicates-rs/blob/master/release.toml):
```toml
pre-release-replacements = [
  {file="README.md", search="predicates = .*", replace="{{crate_name}} = \"{{version}}\""},
  {file="src/lib.rs", search="predicates = .*", replace="{{crate_name}} = \"{{version}}\""},
]
```

Note: we only substitute variables on `replace` and not `search` so you'll need
to change `predicates` to match your crate name.

See [`pre-release-replacements`](reference.md) for more.

## Maintaining Changelog

At the moment, `cargo release` won't try automatically generate a changelog from
the git history, because I think changelog is an important communication between
developer and users, which requires careful maintenance.

For small solo projects where there is enough information in the commit history
to communicate well with the user, you might want to setup a pre-release hook
using [git-cliff](https://github.com/orhun/git-cliff) which expects a version
number to be passed to on the command line. Don't be tempted to use both a hook
and replacements as the pre release hook occurs after the replacements step of
the release process. The following is configuration will let git-cliff update
the CHANGELOG.md file prior as part of the tagged commit which updates the
version in the cargo.toml file.

```toml
pre-release-hook = ["git", "cliff", "-o", "CHANGELOG.md", "--tag", "{{version}}" ]
```

However, if instead you're maintaining a changelog incrementally while developing,
you can still use [`pre-release-replacements`](reference.md) to smooth your
process of releasing a changelog, along with your crate. You need to
keep your changelog arranged during feature development, in an `Unreleased`
section (recommended by [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)):

```markdown
<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added
- feature 3

### Changed
- bug 1

## [1.0.0] - 2017-06-20
### Added
- feature 1
- feature 2

<!-- next-url -->
[Unreleased]: https://github.com/assert-rs/predicates-rs/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/assert-rs/predicates-rs/compare/v0.9.0...v1.0.0
```

In `release.toml`, configure `cargo release` to do replacements while
bumping version:

```toml
pre-release-replacements = [
  {file="CHANGELOG.md", search="Unreleased", replace="{{version}}"},
  {file="CHANGELOG.md", search="\\.\\.\\.HEAD", replace="...{{tag_name}}", exactly=1},
  {file="CHANGELOG.md", search="ReleaseDate", replace="{{date}}"},
  {file="CHANGELOG.md", search="<!-- next-header -->", replace="<!-- next-header -->\n\n## [Unreleased] - ReleaseDate", exactly=1},
  {file="CHANGELOG.md", search="<!-- next-url -->", replace="<!-- next-url -->\n[Unreleased]: https://github.com/assert-rs/predicates-rs/compare/{{tag_name}}...HEAD", exactly=1},
]
```

`{{version}}` and `{{date}}` are pre-defined variables with value of
current release version and date.

`predicates` is a real world example
- [`release.toml`](https://github.com/assert-rs/predicates-rs/blob/master/release.toml)
- [`CHANGELOG.md`](https://github.com/assert-rs/predicates-rs/blob/master/CHANGELOG.md)

## How do I apply a setting only to one crate in my workspace?

Example problems:
- Release fails because `cargo-release` was trying to update a non-existent `CHANGELOG.md` ([#157](https://github.com/crate-ci/cargo-release/issues/157))
- Only create one tag for the entire workspace ([#162](https://github.com/crate-ci/cargo-release/issues/162))

Somethings only need to be done on for a release, like updating the
`CHANGELOG.md`, no matter how many crates are being released.  Usually these
operations are tied to a specific crate.  This is straightforward when you do
not have a crate in your workspace root.

When you have a root crate, it shares its `release.toml` with the workspace,
making it less obvious how to do root-crate-specific settings.   If you'd like
to customize settings differently for the root crate vs the other crate's, you
have two choices:
- Put the common setting in the workspace's `release.toml` and override it for the root crate in `Cargo.toml`.
- Modify each crate's `release.toml` with the setting relevant for that crate.

## How do I customize my tagging in a workspace?

Example problems:
- Customizing tags while needing the root workspace to follow a specific convention ([#172](https://github.com/crate-ci/cargo-release/issues/172))

By default, your tag will look like:
- `v{{version}}` if the crate is in the repo root.
- `{{crate_name}}-v{{version}}` otherwise.

This is determined by `tag-name` with the default `"{{prefix}}v{{version}}"`.
- `"{{prefix}}"` comes from `tag-prefix` which has two defaults:
  - `""` if the crate is in the repo root.
  - `"{{crate_name}}-"` otherwise.
- `"{{version}}"` is the current crate's version.

Other variables are available for use.  See the [reference](reference.md) for more.

`tag-name`, `tag-prefix` come from the config file and `cargo-release` uses a layered config.  The relevant layers are:
1. Read from the crate's `Cargo.toml`
2. Read from the crate's `release.toml`
3. Read from the workspace's `release.toml`.

Something to keep in mind is if you have a crate in your workspace root, it
shares the `release.toml` with the workspace.  If you'd like to customize the
tag differently for the root crate vs the other crate's, you have two choices:
- Put the common setting in the workspace's `release.toml` and override it for the root crate in `Cargo.toml`.
- Modify each crate's `release.toml` with the setting relevant for that crate.

## How do I do a release when there is dependency cycle in my workspace?

If this is for dev-dependencies, just declare your dev-dependency with only a path, no version, and it should work out.

If you have other cycles, open an issue, we'd love to hear about your use case and see how we can help!

## Why does `cargo-release` say a package has changes and needs to be released?

If you run with extra logging, we'll call out which file changed that triggered the release.

If that file shouldn't be included in the package, update your `Cargo.toml`'s
[`include` and `exclude` fields](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields).

## How do I automate creating a Release on Github

We recommend creating a workflow that creates a Release based on tags being published
- [Hand-written example](https://github.com/crate-ci/typos/commit/5c92dc6f8cc68b9d1c8cb1e8840b81e78cf7f65d)
- Pre-built [github-release-action](https://github.com/taiki-e/create-gh-release-action)

## How do I support [`cargo binstall`](https://crates.io/crates/cargo-binstall)

See [cargo nextest](https://github.com/nextest-rs/nextest/blob/main/internal-docs/releasing.md) as an example workflow for this
