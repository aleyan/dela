use colored::*;

/// Color scheme for task names
pub fn task_name_normal() -> ColoredString {
    "".green()
}

pub fn task_name_ambiguous() -> ColoredString {
    "".dimmed().red()
}

pub fn task_name_shadowed() -> ColoredString {
    "".dimmed().red()
}

/// Color scheme for footnotes
pub fn footnote_symbol() -> ColoredString {
    "".yellow()
}

pub fn footnote_description() -> ColoredString {
    "".dimmed()
}

/// Color scheme for task runners
pub fn task_runner_available() -> ColoredString {
    "".cyan().bold()
}

pub fn task_runner_unavailable() -> ColoredString {
    "".red()
}

/// Color scheme for task definition files
pub fn task_definition_file() -> ColoredString {
    "".dimmed()
}

/// Color scheme for section counts
pub fn section_count() -> ColoredString {
    "".blue()
}

/// Color scheme for task descriptions
pub fn task_description() -> ColoredString {
    "".white()
}

pub fn task_description_dash() -> ColoredString {
    "-".dimmed()
}

/// Color scheme for status indicators
pub fn status_success() -> ColoredString {
    "✓".green()
}

pub fn status_warning() -> ColoredString {
    "!".yellow()
}

pub fn status_error() -> ColoredString {
    "✗".red()
}

pub fn status_not_found() -> ColoredString {
    "-".dimmed()
}

/// Color scheme for error messages
pub fn error_header() -> ColoredString {
    "".red().bold()
}

pub fn error_bullet() -> ColoredString {
    "•".red()
}

pub fn error_message() -> ColoredString {
    "".red()
}

/// Color scheme for informational messages
pub fn info_message() -> ColoredString {
    "".yellow()
}

pub fn info_header() -> ColoredString {
    "".dimmed()
}
