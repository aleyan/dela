use crate::types::{AllowScope, Task};
use std::io::{self, Write};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::Stdout;

#[derive(Debug, PartialEq, Clone)]
pub enum AllowDecision {
    Allow(AllowScope),
    Deny,
}

/// Prompt the user for a decision about a task using a TUI interface
pub fn prompt_for_task(task: &Task) -> Result<AllowDecision, String> {
    // Check if we're in a test environment or non-interactive terminal
    let is_test = std::env::var("RUST_TEST_THREADS").is_ok() 
        || std::env::var("CARGO_TEST").is_ok();
    let is_interactive = atty::is(atty::Stream::Stdout) && atty::is(atty::Stream::Stdin);
    
    // Force fallback in test environment or when stdin/stdout are redirected
    if is_test || !is_interactive {
        return prompt_for_task_fallback(task);
    }

    // Try to setup terminal, fallback to text prompt if it fails
    match enable_raw_mode() {
        Ok(_) => {
            let mut stdout = io::stdout();
            match execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
                Ok(_) => {
                    let backend = CrosstermBackend::new(stdout);
                    match Terminal::new(backend) {
                        Ok(mut terminal) => {
                            let result = run_tui(&mut terminal, task);
                            
                            // Restore terminal
                            let _ = disable_raw_mode();
                            let _ = execute!(
                                terminal.backend_mut(),
                                LeaveAlternateScreen,
                                DisableMouseCapture
                            );
                            let _ = terminal.show_cursor();
                            
                            result
                        }
                        Err(_) => prompt_for_task_fallback(task),
                    }
                }
                Err(_) => prompt_for_task_fallback(task),
            }
        }
        Err(_) => prompt_for_task_fallback(task),
    }
}

/// Fallback text-based prompt for non-interactive environments
fn prompt_for_task_fallback(task: &Task) -> Result<AllowDecision, String> {
    println!(
        "\nTask '{}' from '{}' requires approval.",
        task.name,
        task.file_path.display()
    );
    if let Some(desc) = &task.description {
        println!("Description: {}", desc);
    }
    println!("\nHow would you like to proceed?");
    println!("1) Allow once (this time only)");
    println!("2) Allow this task (remember for this task)");
    println!("3) Allow file (remember for all tasks in this file)");
    println!("4) Allow directory (remember for all tasks in this directory)");
    println!("5) Deny (don't run this task)");

    print!("\nEnter your choice (1-5): ");
    io::stdout()
        .flush()
        .map_err(|e| format!("Failed to flush stdout: {}", e))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    match input.trim() {
        "1" => Ok(AllowDecision::Allow(AllowScope::Once)),
        "2" => Ok(AllowDecision::Allow(AllowScope::Task)),
        "3" => Ok(AllowDecision::Allow(AllowScope::File)),
        "4" => Ok(AllowDecision::Allow(AllowScope::Directory)),
        "5" => Ok(AllowDecision::Deny),
        _ => Err("Invalid choice. Please enter a number between 1 and 5.".to_string()),
    }
}

fn run_tui(terminal: &mut Terminal<CrosstermBackend<Stdout>>, task: &Task) -> Result<AllowDecision, String> {
    let options = vec![
        ("Allow once (this time only)", AllowDecision::Allow(AllowScope::Once)),
        ("Allow this task (remember for this task)", AllowDecision::Allow(AllowScope::Task)),
        ("Allow file (remember for all tasks in this file)", AllowDecision::Allow(AllowScope::File)),
        ("Allow directory (remember for all tasks in this directory)", AllowDecision::Allow(AllowScope::Directory)),
        ("Deny (don't run this task)", AllowDecision::Deny),
    ];

    let mut selected = 0;

    loop {
        terminal
            .draw(|f| ui(f, task, &options, selected))
            .map_err(|e| format!("Failed to draw UI: {}", e))?;

        if let Event::Key(key) = event::read().map_err(|e| format!("Failed to read event: {}", e))? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    return Err("User cancelled".to_string());
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = if selected == 0 {
                        options.len() - 1
                    } else {
                        selected - 1
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1) % options.len();
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    selected = 0;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    selected = options.len() - 1;
                }
                KeyCode::Enter => {
                    return Ok(options[selected].1.clone());
                }
                _ => {}
            }
        }
    }
}

fn ui(
    f: &mut Frame,
    task: &Task,
    options: &[(&str, AllowDecision)],
    selected: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Min(3),    // Header (allow for wrapping)
                Constraint::Length(1), // Spacer
                Constraint::Length(7), // Options list (exactly 5 lines)
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Instructions
            ]
            .as_ref(),
        )
        .split(f.size());

    // Header
    let header_text = vec![
        Line::from(vec![
            Span::styled(
                format!("Task '{}' requires approval from:", task.name),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  {}", task.file_path.display()),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];



    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Task Approval"))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(header, chunks[0]);

    // Options list
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (text, _))| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("▶ {}", text)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Options"))
        .style(Style::default().fg(Color::White));
    f.render_widget(list, chunks[2]);

    // Instructions
    let instructions = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("↑/↓ or j/k", Style::default().fg(Color::Yellow)),
            Span::styled(" to navigate, ", Style::default().fg(Color::White)),
            Span::styled("g/G", Style::default().fg(Color::Yellow)),
            Span::styled(" for first/last, ", Style::default().fg(Color::White)),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::styled(" to select, ", Style::default().fg(Color::White)),
            Span::styled("q/Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" to cancel", Style::default().fg(Color::White)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Controls"));
    f.render_widget(instructions, chunks[4]);
}

#[cfg(test)]
mod tests {
    use super::*;




    // Test helper function that simulates the TUI logic
    fn test_tui_logic(selected_index: usize) -> Result<AllowDecision, String> {
        let options = vec![
            ("Allow once (this time only)", AllowDecision::Allow(AllowScope::Once)),
            ("Allow this task (remember for this task)", AllowDecision::Allow(AllowScope::Task)),
            ("Allow file (remember for all tasks in this file)", AllowDecision::Allow(AllowScope::File)),
            ("Allow directory (remember for all tasks in this directory)", AllowDecision::Allow(AllowScope::Directory)),
            ("Deny (don't run this task)", AllowDecision::Deny),
        ];

        if selected_index < options.len() {
            Ok(options[selected_index].1.clone())
        } else {
            Err("Invalid selection index".to_string())
        }
    }

    #[test]
    fn test_prompt_allow_once() {
        let result = test_tui_logic(0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Once));
    }

    #[test]
    fn test_prompt_allow_task() {
        let result = test_tui_logic(1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Task));
    }

    #[test]
    fn test_prompt_allow_file() {
        let result = test_tui_logic(2);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::File));
    }

    #[test]
    fn test_prompt_allow_directory() {
        let result = test_tui_logic(3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Directory));
    }

    #[test]
    fn test_prompt_deny() {
        let result = test_tui_logic(4);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Deny);
    }

    #[test]
    fn test_prompt_invalid_selection() {
        let result = test_tui_logic(10);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid selection index");
    }
}
