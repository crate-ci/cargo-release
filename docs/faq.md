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

At the moment, `cargo release` is unopinionated in its support for CHANGELOGs
due to the complexities and the different approaches people might want to take
(see [Issue #231](https://github.com/crate-ci/cargo-release/issues/231)).

As a CHANGELOG is better than no changelog, a low-effort approach would be to
use
[git-cliff](https://github.com/orhun/git-cliff) as a pre-release hook.
```toml
pre-release-hook = ["git", "cliff", "-o", "CHANGELOG.md", "--tag", "{{version}}" ]
```

For hand-written CHANGELOGs, you can automate parts of the process with
[`pre-release-replacements`](reference.md).  Say you follow
[Keep a Changelog](http://keepachangelog.com/en/1.0.0/) and keep unreleased
changes in an `Unreleased` section:
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
  {file="CHANGELOG.md", search="<!-- next-url -->", replace="<!-- next-url -->\n[Unreleased]: {{repository}}/compare/{{tag_name}}...HEAD", exactly=1},
]
```

Pre-defined variables used:

- `{{version}}` is the current release version
- `{{date}}` is the current date
- `{{repository}}` is the `repository` field in `Cargo.toml`, in this case it is `https://github.com/assert-rs/predicates-rs`
- `{{tag_name}}` is the tag being published

`{{version}}` and `{{date}}` are pre-defined variables with value of
current release version and date.

`predicates` is a real world example
- [`release.toml`](https://github.com/assert-rs/predicates-rs/blob/master/release.toml)
- [`CHANGELOG.md`](https://github.com/assert-rs/predicates-rs/blob/master/CHANGELOG.md)

## How do I apply a setting only to one crate in my workspace?

Example problems:
- Release fails because `cargo-release` was trying to update a non-existent `CHANGELOG.md` ([#157](https://github.com/crate-ci/cargo-release/issues/157))
- Only create one tag for the entire workspace ([#162](https://github.com/crate-ci/cargo-release/issues/162))
- Hooks are run multiple times, rather than once ([#925](https://github.com/crate-ci/cargo-release/issues/925))

Some things may only need to be done once for a release
no matter how many crates are being released,
like updating the `CHANGELOG.md`.
It is important to know that `cargo release` operates on mostly packages.
Unless specified otherwise,
configuration on a workspace is for being inherited by packages,
including hooks, replacements, and tagging.

We recommend picking a "representative" package and make it responsible for performing these operations
by putting this once-per-release configuration in its
[package-specific configuration](reference.md#sources).

When you have a root crate in your workspace,
`release.toml` is shared between the package and the workspace.
Prefer `Cargo.toml`s `[package.metadata.release]` for package specific configuration to avoid this problem.

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
