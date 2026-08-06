use cargo_test_support::cargo_test;
use cargo_test_support::project;
use snapbox::str;

use crate::CargoCommand;
use crate::create_default_gitconfig;
use crate::init_registry;

#[cargo_test]
fn unpublished_workspace_dependency() {
    let registry = init_registry();
    create_default_gitconfig();
    let project = project()
        .file(".gitignore", "/target\n")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            resolver = "2"
            members = ["dependency", "application"]
            "#,
        )
        .file(
            "dependency/Cargo.toml",
            r#"
            [package]
            name = "dependency"
            version = "0.1.0"
            edition = "2024"
            description = "An unpublished dependency"
            license = "MIT"
            repository = "https://example.com"
            "#,
        )
        .file("dependency/src/lib.rs", "pub fn dependency() {}")
        .file(
            "application/Cargo.toml",
            r#"
            [package]
            name = "application"
            version = "0.1.0"
            edition = "2024"
            publish = ["dummy-registry"]
            description = "An application"
            license = "MIT"
            repository = "https://example.com"

            [dependencies]
            dependency = { path = "../dependency", version = "0.1.0" }
            "#,
        )
        .file("application/src/lib.rs", "pub fn application() {}")
        .build();
    project.process("cargo").arg("generate-lockfile").run();
    let repo = cargo_test_support::git::init(&project.root());
    cargo_test_support::git::add(&repo);
    cargo_test_support::git::commit(&repo);

    snapbox::cmd::Command::cargo_ui()
        .arg("release")
        .args(["--package", "application", "--execute", "--no-confirm"])
        .current_dir(project.root())
        .env("CARGO_REGISTRIES_DUMMY_REGISTRY_TOKEN", registry.token())
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
warning: disabled by user, skipping dependency v0.1.0 despite being unpublished
error: application 0.1.0 depends on unpublished workspace package dependency 0.1.0

"#]]);
}

#[cargo_test]
fn unpublished_git_dependency() {
    let registry = init_registry();
    create_default_gitconfig();
    let dependency = cargo_test_support::git::new("dependency", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                [package]
                name = "dependency"
                version = "0.1.0"
                edition = "2024"
                "#,
            )
            .file("src/lib.rs", "pub fn dependency() {}")
    });
    let git_dev_dependency = cargo_test_support::git::new("git-dev-dependency", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "git-dev-dependency"
                    version = "0.1.0"
                    edition = "2024"
                    "#,
            )
            .file("src/lib.rs", "pub fn git_dev_dependency() {}")
    });
    let manifest = format!(
        r#"
        [package]
        name = "application"
        version = "0.1.0"
        edition = "2024"
        publish = ["dummy-registry"]
        description = "An application"
        license = "MIT"
        repository = "https://example.com"

        [dependencies]
        dependency = {{ git = "{dependency_url}", version = "0.1.0" }}

        [dev-dependencies]
        git-dev-dependency = {{ git = "{git_dev_dependency_url}" }}
        path-dev-dependency = {{ path = "path-dev-dependency" }}
        "#,
        dependency_url = dependency.url(),
        git_dev_dependency_url = git_dev_dependency.url(),
    );
    let project = project()
        .file(".gitignore", "/target\n")
        .file("Cargo.toml", &manifest)
        .file(
            "path-dev-dependency/Cargo.toml",
            r#"
            [package]
            name = "path-dev-dependency"
            version = "0.1.0"
            edition = "2024"
            "#,
        )
        .file(
            "path-dev-dependency/src/lib.rs",
            "pub fn path_dev_dependency() {}",
        )
        .file("src/lib.rs", "pub fn application() {}")
        .build();
    project.process("cargo").arg("generate-lockfile").run();
    let repo = cargo_test_support::git::init(&project.root());
    cargo_test_support::git::add(&repo);
    cargo_test_support::git::commit(&repo);

    snapbox::cmd::Command::cargo_ui()
        .arg("release")
        .args(["--package", "application", "--execute", "--no-confirm"])
        .current_dir(project.root())
        .env("CARGO_REGISTRIES_DUMMY_REGISTRY_TOKEN", registry.token())
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
error: application 0.1.0 depends on unpublished package dependency ^0.1.0

"#]]);
}
