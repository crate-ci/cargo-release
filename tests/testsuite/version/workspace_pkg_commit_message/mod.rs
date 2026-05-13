use cargo_test_support::cargo_test;
use cargo_test_support::current_dir;

use crate::CargoCommand;
use crate::git_from;
use crate::init_registry;

#[cargo_test]
fn case() {
    init_registry();
    let project = git_from(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("release")
        .args([
            "--package",
            "a",
            "--package",
            "b",
            "0.2.0",
            "-x",
            "--no-confirm",
            "--no-publish",
            "--no-tag",
            "--no-push",
        ])
        .current_dir(cwd)
        .assert()
        .success();

    let repo = git2::Repository::open(&project_root)
        .expect("failed to open git repo");
    let head_commit = repo
        .head()
        .expect("failed to read HEAD")
        .peel_to_commit()
        .expect("HEAD is not a commit");
    let commit_msg = head_commit
        .message()
        .expect("commit message is not valid UTF-8")
        .trim();
    assert_eq!(commit_msg, "chore: Release version 0.2.0");
}
