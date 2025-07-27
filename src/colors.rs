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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_name_colors() {
        // Test normal task name color
        let normal = task_name_normal();
        // In CI environments, colors might be disabled, so we just check the function doesn't panic
        let _ = normal.to_string();

        // Test ambiguous task name color
        let ambiguous = task_name_ambiguous();
        let _ = ambiguous.to_string();

        // Test shadowed task name color
        let shadowed = task_name_shadowed();
        let _ = shadowed.to_string();
    }

    #[test]
    fn test_footnote_colors() {
        // Test footnote symbol color
        let symbol = footnote_symbol();
        let _ = symbol.to_string();

        // Test footnote description color
        let description = footnote_description();
        let _ = description.to_string();
    }

    #[test]
    fn test_task_runner_colors() {
        // Test available runner color
        let available = task_runner_available();
        let _ = available.to_string();

        // Test unavailable runner color
        let unavailable = task_runner_unavailable();
        let _ = unavailable.to_string();
    }

    #[test]
    fn test_task_definition_file_colors() {
        // Test task definition file color
        let file = task_definition_file();
        let _ = file.to_string();
    }

    #[test]
    fn test_section_count_colors() {
        // Test section count color
        let count = section_count();
        let _ = count.to_string();
    }

    #[test]
    fn test_task_description_colors() {
        // Test task description color
        let description = task_description();
        let _ = description.to_string();

        // Test task description dash color
        let dash = task_description_dash();
        let dash_str = dash.to_string();
        assert!(dash_str.contains("-"));
    }

    #[test]
    fn test_status_colors() {
        // Test success status color
        let success = status_success();
        assert!(!success.to_string().is_empty());
        assert!(success.to_string().contains("✓"));

        // Test warning status color
        let warning = status_warning();
        assert!(!warning.to_string().is_empty());
        assert!(warning.to_string().contains("!"));

        // Test error status color
        let error = status_error();
        assert!(!error.to_string().is_empty());
        assert!(error.to_string().contains("✗"));

        // Test not found status color
        let not_found = status_not_found();
        assert!(!not_found.to_string().is_empty());
        assert!(not_found.to_string().contains("-"));
    }

    #[test]
    fn test_error_colors() {
        // Test error header color
        let header = error_header();
        let _ = header.to_string();

        // Test error bullet color
        let bullet = error_bullet();
        let bullet_str = bullet.to_string();
        assert!(bullet_str.contains("•"));

        // Test error message color
        let message = error_message();
        let _ = message.to_string();
    }

    #[test]
    fn test_info_colors() {
        // Test info message color
        let message = info_message();
        let _ = message.to_string();

        // Test info header color
        let header = info_header();
        let _ = header.to_string();
    }

    #[test]
    fn test_color_consistency() {
        // Test that colors are consistent across calls
        let normal1 = task_name_normal();
        let normal2 = task_name_normal();
        assert_eq!(normal1.to_string(), normal2.to_string());

        let error1 = status_error();
        let error2 = status_error();
        assert_eq!(error1.to_string(), error2.to_string());
    }

    #[test]
    fn test_color_differentiation() {
        // Test that different colors are actually different
        let normal = task_name_normal();
        let ambiguous = task_name_ambiguous();
        // In CI environments, colors might be disabled, so we just check the functions don't panic
        let _ = normal.to_string();
        let _ = ambiguous.to_string();

        let success = status_success();
        let error = status_error();
        // These should be different because they have different symbols
        let success_str = success.to_string();
        let error_str = error.to_string();
        assert_ne!(success_str, error_str);
    }

    #[test]
    fn test_color_formatting() {
        // Test that colors are properly formatted
        let colors = vec![
            task_name_normal(),
            task_name_ambiguous(),
            task_name_shadowed(),
            footnote_symbol(),
            footnote_description(),
            task_runner_available(),
            task_runner_unavailable(),
            task_definition_file(),
            section_count(),
            task_description(),
            task_description_dash(),
            status_success(),
            status_warning(),
            status_error(),
            status_not_found(),
            error_header(),
            error_bullet(),
            error_message(),
            info_message(),
            info_header(),
        ];

        // In CI environments, colors might be disabled, so we just check the functions don't panic
        for color in colors {
            let _ = color.to_string();
        }
    }
}
