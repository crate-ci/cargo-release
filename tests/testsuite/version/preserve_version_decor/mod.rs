use cargo_test_support::cargo_test;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;

use crate::CargoCommand;
use crate::git_from;
use crate::init_registry;

#[cargo_test]
fn case() {
    init_registry();
    let project = git_from(current_dir!().join("in"));
    let project_root = project.root();

    snapbox::cmd::Command::cargo_ui()
        .arg("release")
        .args(["version", "2.0.0", "--workspace", "-x", "--no-confirm"])
        .current_dir(&project_root)
        .assert()
        .success();

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
