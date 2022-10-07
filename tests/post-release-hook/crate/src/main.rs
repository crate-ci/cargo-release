use std::env;

fn main() {
    eprintln!("START_ENV_VARS");
    for (key, value) in env::vars_os() {
        eprintln!("{}={}", key.to_string_lossy(), value.to_string_lossy());
    }
    eprintln!("END_ENV_VARS");
}
