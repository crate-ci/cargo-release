use cargo_test_support::cargo_test;
use cargo_test_support::current_dir;

use crate::CargoCommand;
use crate::git_from;

#[cargo_test]
fn case() {
    let project = git_from(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    let output = snapbox::cmd::Command::cargo_ui()
        .arg("release")
        .args(["publish", "-x", "--workspace", "--registry", "local"])
        .stdin("n\n")
        .current_dir(cwd)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected publish confirmation decline to exit successfully\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("cargo-release-public-fixture"),
        "expected publishable package in confirmation prompt\nstdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("cargo-release-private-fixture"),
        "did not expect publish=false package in confirmation prompt\nstdout:\n{stdout}"
    );
}
