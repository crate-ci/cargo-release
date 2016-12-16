# cargo release

[![](http://meritbadge.herokuapp.com/cargo-release)](https://crates.io/crates/cargo-release)
[![Build Status](https://travis-ci.org/sunng87/cargo-release.svg?branch=master)](https://travis-ci.org/sunng87/cargo-release)

This a script standardize release process of cargo project for you.

Basically it runs following tasks:

* Check if current working directory is git clean
* Read version from Cargo.toml, remove pre-release extension, bump
  version and commit if necessary
* Run `cargo publish`
* Generate rustdoc and push to gh-pages optionally
* Create a git tag for this version
* Bump version for next development cycle
* `git push`

## Install

`cargo install cargo-release`

## Usage

`cargo release`

### Prerequisite

* Your project should be managed by git.

### Release level

Use `-l [level]` or `--level [level]` to specify a release level.

* By default, cargo release removes pre-release extension; if there is
no pre-release extension, the current version will be used (0.1.0-pre
-> 0.1.0, 0.1.0 -> 0.1.0)
* If level is `patch` and current version is a pre-release, it behaves
like default; if current version has no extension, it bumps patch
version (0.1.0 -> 0.1.1)
* If level is `minor`, it bumps minor version (0.1.0-pre -> 0.2.0)
* If level is `major`, it bumps major version (0.1.0-pre -> 1.0.0)

From 0.7, you can also use `alpha`, `beta` and `rc` for `level`. It
adds pre-release to your version. You can have multiple `alpha`
version as it goes to `alpha.1`, `alpha.2`…

Releasing `alpha` version on top of a `beta` or `rc` version is not
allowed and will be resulted in error. So does `beta` on `rc`. It is
recommended to use `--dry-run` if you are not sure about the behavior
of specific `level`.

### Signing your git commit and tag

Use `--sign` option to GPG sign your release commits and
tags. [Further
information](https://git-scm.com/book/en/v2/Git-Tools-Signing-Your-Work)

### Upload rust doc to github pages

By using `--upload-doc` option, cargo-release will generate rustdoc
during release process, and commit the doc directory to `gh-pages`
branch. So you can access your rust doc at
https://YOUR-GITHUB-USERNAME.github.io/YOUR-REPOSITORY-NAME/YOUR-CRATE-NAME

If your hosting service uses different branch for pages, you can use
`--doc-branch` to customize the branch we push docs to.

If you only want to use cargo release for uploading docs to `gh-pages`, you
can use `--upload-doc-only` option. This would typically be done on the
command-line:

```
cargo release --upload-doc-only
```

#### WARNING

This option will override your existed doc branch,
use it at your own risk.

### Tag prefix

For single-crate repository, we will use version number as git tag
name.

For multi-crate repository, the subdirectory name will be used as tag
name. For example, when releasing serde_macros 0.7.0 in serde-rs/serde
repo, a tag named as `serde_macros-0.7.0` will be created.

You can always override this behavior by using `--tag-prefix <prefix>`
option.

### Custom remote to push

In case your `origin` is not writable, you can specify custom remote
by `--push-remote` to set the remote to push.

Use `--skip-push` if you do not plan to push to anywhere for now.

### Specifying dev pre-release extension

After release, the version in Cargo.toml will be incremented and have
a pre-release extension added, defaulting to `pre`.

You can specify a different extension by using the
`--dev-version-ext <ext>` option. To disable version bump after
release, use `--no-dev-version` option.

### Configuration in Cargo.toml

From 0.6 you can persist options above in `Cargo.toml`. We use a
custom section called `package.metadata.release` in `Cargo.toml` to
store these options. Available keys:

* `sign-commit`: bool, use GPG to sign git commits and tag generated by
  cargo-release
* `upload-doc`: bool, generate doc and push to remote branch
* `upload-doc-only`: bool, onlygenerate doc and push to remote branch
* `doc-branch`: string, default branch to push docs
* `push-remote`: string, default git remote to push
* `disable-push`: bool, don't do git push
* `dev-version-ext`: string, pre-release extension to use on the next
  development version.
* `pre-release-commit-message`: string, a commit message template for
  release. For example: `"release {{version}}"`, where `{{version}}`
  will be replaced by actual version.
* `pro-release-commit-message`: string, a commit message template for
  bumping version after release. For example: `starting next iteration
  {{version}}`, where `{{version}}` will be replaced by actual
  version.
* `tag-message`: string, a message template for tag. Available
  variables: `{{version}}`, `{{prefix}}` (the tag prefix)
* `doc-commit-message`: string, a commit message template for doc
  import.
* `no-dev-version`: bool, disable version bump after release.

```toml
[package.metadata.release]
sign-commit = true
upload-doc = true
pre-release-commit-message = "Release {{version}} 🎉🎉"
```

### Dry run

Always call `cargo release --dry-run` with your custom options before
actually executing it. The dry-run mode will print all commands to
execute during the release process. And you will get an overview of
what's going on.

Here is an example.

```
 $ cargo release --dry-run
cd .
git commit -S -am (cargo-release) version 0.18.3
cd -
cargo publish
Building and exporting docs.
cargo doc --no-deps
cd target/doc/
git init
cd -
cd target/doc/
git add .
cd -
cd target/doc/
git commit -S -am (cargo-release) generate docs
cd -
cd target/doc/
git push -f git@github.com:sunng87/handlebars-rust.git master:gh-pages
cd -
git tag -a 0.18.3 -m (cargo-release)  version 0.18.3 -s
Starting next development iteration 0.18.4-pre
cd .
git commit -S -am (cargo-release) start next development iteration 0.18.4-pre
cd -
git push origin --follow-tags
```

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
