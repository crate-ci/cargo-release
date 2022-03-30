use assert_fs::prelude::*;
use assert_fs::TempDir;
use maplit::hashset;
use std::collections::HashSet;
use std::env;
use std::fmt::Debug;
use std::path::Path;
use std::process::{Command, Stdio};
use std::str::Lines;

#[test]
fn krate() {
    let root = Path::new("tests/post-release-hook/crate")
        .canonicalize()
        .unwrap();
    let expected = hashset! {
        EnvVar::new("RELEASE_TAG", "v1.2.3"),
        EnvVar::new("DRY_RUN", "true"),
    };
    let output = run_cargo_release(root.as_path());
    let mut output = output.lines();
    let actual = read_env_var_dump(&mut output).expect("missing env var dump");
    assert!(
        actual.is_superset(&expected),
        "not all expected env vars are present and matching\n{expected:#?}"
    );
}

#[test]
fn workspace() {
    let ws_root = Path::new("tests/post-release-hook/workspace")
        .canonicalize()
        .unwrap();
    let expected = vec![
        hashset! {
            EnvVar::new("RELEASE_TAG", "post-release-hook-ws-a-v42.42.42"),
            EnvVar::new("DRY_RUN", "true"),
        },
        hashset! {
            EnvVar::new("RELEASE_TAG", "post-release-hook-ws-b-v420.420.420"),
            EnvVar::new("DRY_RUN", "true"),
        },
    ];
    let output = run_cargo_release(ws_root.as_path());
    let mut output = output.lines();
    let mut actual = Vec::new();
    while let Some(vars) = read_env_var_dump(&mut output) {
        actual.push(vars);
    }
    for expected in expected {
        assert!(actual.iter().any(|actual| actual.is_superset(&expected)));
    }
}

fn run_cargo_release(dir: &Path) -> String {
    let temp = TempDir::new().unwrap();
    temp.copy_from(dir, &["**"]).unwrap();

    git(temp.path(), &["init"]);
    git(temp.path(), &["add", "."]);
    git(
        temp.path(),
        &["commit", "--message", "this is a commit message"],
    );

    let mut cargo = env::var_os("CARGO").map_or_else(|| Command::new("cargo"), Command::new);
    let output = cargo
        .stderr(Stdio::piped())
        .args(["run", "--manifest-path"])
        .arg(Path::new(PROJECT_ROOT).join("Cargo.toml"))
        .args(["--", "release", "-vv"])
        .current_dir(&temp)
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();

    temp.close().unwrap();

    if !output.status.success() {
        panic!("cargo release exited with {}", output.status);
    }
    let output = String::from_utf8(output.stderr).unwrap();
    eprintln!("{output}");
    output
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(args)
        .current_dir(dir)
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success());
}

fn read_env_var_dump(lines: &mut Lines) -> Option<HashSet<EnvVar>> {
    let mut variables = HashSet::new();
    loop {
        if lines.next()?.trim() == "START_ENV_VARS" {
            break;
        }
    }
    loop {
        let line = lines.next().expect("missing end of env var dump").trim();
        if line == "END_ENV_VARS" {
            return Some(variables);
        }

        let (key, value) = line.split_once('=').unwrap();
        variables.insert(EnvVar::new(key, value));
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct EnvVar {
    key: String,
    value: String,
}

impl EnvVar {
    fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }
}

const PROJECT_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"));
