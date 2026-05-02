#![warn(clippy::needless_borrow)]
#![warn(clippy::redundant_clone)]

mod version;

fn init_registry() {
    cargo_test_support::registry::init();
}

pub fn git_from(template: impl AsRef<std::path::Path>) -> cargo_test_support::Project {
    create_default_gitconfig();
    let project = cargo_test_support::Project::from_template(template.as_ref());
    project.process("cargo").arg("generate-lockfile").run();
    let repo = cargo_test_support::git::init(&project.root());
    cargo_test_support::git::add(&repo);
    cargo_test_support::git::commit(&repo);
    project
}

fn create_default_gitconfig() {
    // If we're running this under a user account that has a different default branch set up
    // then tests that assume the default branch is master will fail. We set the default branch
    // to master explicitly so that tests that rely on this behavior still pass.
    let gitconfig = cargo_test_support::paths::home().join(".gitconfig");
    std::fs::write(
        gitconfig,
        r#"
        [init]
            defaultBranch = master
        "#,
    )
    .unwrap();
}

pub fn cargo_exe() -> std::path::PathBuf {
    snapbox::cmd::cargo_bin("cargo-release")
}

/// Test the cargo command
pub trait CargoCommand {
    fn cargo_ui() -> Self;
}

impl CargoCommand for snapbox::cmd::Command {
    fn cargo_ui() -> Self {
        use cargo_test_support::TestEnvCommandExt;
        Self::new(cargo_exe())
            .env("CARGO_TERM_UNICODE", "true")
            .with_assert(cargo_test_support::compare::assert_ui())
            .test_env()
    }
}
