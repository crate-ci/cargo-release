use std::io::{Write, stdin, stdout};

use anyhow::Context as _;
use clap::builder::styling::Style;
use clap_cargo::style::HEADER;

use crate::error::CargoResult;

pub fn confirm(prompt: &str) -> bool {
    let mut input = String::new();

    console_println(&format!("{prompt} [y/N] "), Style::new());

    stdout().flush().unwrap();
    stdin().read_line(&mut input).expect("y/n required");

    input.trim().to_lowercase() == "y"
}

fn console_println(text: &str, style: Style) {
    let _ = writeln!(anstream::stdout(), "{style}{text}{style:#}");
}

/// Print a message with a colored title in the style of Cargo shell messages.
pub fn print(
    status: &str,
    message: impl std::fmt::Display,
    style: Style,
    justified: bool,
) -> CargoResult<()> {
    let mut stderr = anstream::stderr().lock();
    if justified {
        write!(stderr, "{style}{status:>12}{style:#}")?;
    } else {
        write!(stderr, "{style}{status}{style:#}:")?;
    }

    writeln!(stderr, " {message:#}").with_context(|| "Failed to write message")?;

    Ok(())
}

/// Prints the passed in [`Report`][annotate_snippets::Report] to stderr
pub fn print_report(report: annotate_snippets::Report<'_>) -> CargoResult<()> {
    let decor_style = if cargo_term_unicode().unwrap_or_else(supports_unicode::supports_unicode) {
        annotate_snippets::renderer::DecorStyle::Unicode
    } else {
        annotate_snippets::renderer::DecorStyle::Ascii
    };
    let rendered = annotate_snippets::Renderer::styled()
        .decor_style(decor_style)
        .render(report);
    let mut stderr = anstream::stderr().lock();
    stderr.write_all(rendered.as_bytes())?;
    stderr.write_all(b"\n")?;
    Ok(())
}

fn cargo_term_unicode() -> Option<bool> {
    std::env::var_os("CARGO_TERM_UNICODE").map(|v| v == "true")
}

/// Print a styled action message.
pub fn status(action: &str, message: impl std::fmt::Display) -> CargoResult<()> {
    print(action, message, HEADER, true)
}

/// Print a styled error message.
pub fn error(message: impl std::fmt::Display) -> CargoResult<()> {
    let report = &[annotate_snippets::Group::with_title(
        annotate_snippets::Level::ERROR.primary_title(message.to_string()),
    )];
    print_report(report)
}

/// Print a styled warning message.
pub fn warn(message: impl std::fmt::Display) -> CargoResult<()> {
    let report = &[annotate_snippets::Group::with_title(
        annotate_snippets::Level::WARNING.primary_title(message.to_string()),
    )];
    print_report(report)
}

pub fn note(message: impl std::fmt::Display) -> CargoResult<()> {
    let report = &[annotate_snippets::Group::with_title(
        annotate_snippets::Level::NOTE.secondary_title(message.to_string()),
    )];
    print_report(report)
}

pub fn help(message: impl std::fmt::Display) -> CargoResult<()> {
    let report = &[annotate_snippets::Group::with_title(
        annotate_snippets::Level::HELP.secondary_title(message.to_string()),
    )];
    print_report(report)
}

pub fn log(level: log::Level, message: impl std::fmt::Display) -> CargoResult<()> {
    match level {
        log::Level::Error => error(message),
        log::Level::Warn => warn(message),
        log::Level::Info => note(message),
        _ => {
            log::log!(level, "{message}");
            Ok(())
        }
    }
}

/// Print a part of a line with formatting
pub fn write_stderr(fragment: impl std::fmt::Display, style: &Style) -> CargoResult<()> {
    write!(anstream::stderr(), "{style}{fragment}{style:#}")?;
    Ok(())
}
