use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};
use std::io::stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use crate::disk::{self, BlockDevice};
use crate::distro::DistroKind;
use crate::install::{InstallConfig, Installer};

#[derive(Debug, Clone, PartialEq)]
enum Screen {
    Welcome,
    DiskSelect,
    Passphrase,
    Confirm,
    Installing,
    Complete,
    Error(String),
}

#[derive(Default)]
struct InstallerState {
    devices: Vec<BlockDevice>,
    selected_device: usize,
    passphrase: String,
    root_password: String,
    install_log: Vec<String>,
    log_receiver: Option<mpsc::Receiver<String>>,
    install_complete: bool,
    install_error: Option<String>,
}

pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(crossterm::terminal::Clear(
        crossterm::terminal::ClearType::All,
    ))?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let result = run_app(&mut terminal).await;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let mut screen = Screen::Welcome;
    let mut state = InstallerState::default();

    loop {
        // Poll for install log messages
        if let Some(ref rx) = state.log_receiver {
            while let Ok(msg) = rx.try_recv() {
                if msg == "__COMPLETE__" {
                    state.install_complete = true;
                    screen = Screen::Complete;
                } else if msg.starts_with("__ERROR__:") {
                    let err = msg.strip_prefix("__ERROR__:").unwrap_or(&msg);
                    state.install_error = Some(err.to_string());
                    screen = Screen::Error(err.to_string());
                } else {
                    state.install_log.push(msg);
                }
            }
        }

        terminal.draw(|f| render(f, &screen, &state))?;

        // Use poll with timeout so we can update the display while installing
        if !crossterm::event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match (&screen, key.code) {
                // Global quit
                (_, KeyCode::Char('q')) if screen != Screen::Installing => break,
                (_, KeyCode::Esc) if screen != Screen::Installing => break,

                // Welcome screen
                (Screen::Welcome, KeyCode::Enter) => {
                    state.devices = disk::list_block_devices()?;
                    screen = Screen::DiskSelect;
                }

                // Disk selection
                (Screen::DiskSelect, KeyCode::Up | KeyCode::Char('k')) => {
                    if state.selected_device > 0 {
                        state.selected_device -= 1;
                    }
                }
                (Screen::DiskSelect, KeyCode::Down | KeyCode::Char('j')) => {
                    if state.selected_device < state.devices.len().saturating_sub(1) {
                        state.selected_device += 1;
                    }
                }
                (Screen::DiskSelect, KeyCode::Enter) => {
                    if !state.devices.is_empty() {
                        screen = Screen::Passphrase;
                    }
                }

                // Passphrase entry
                (Screen::Passphrase, KeyCode::Char(c)) => {
                    state.passphrase.push(c);
                }
                (Screen::Passphrase, KeyCode::Backspace) => {
                    state.passphrase.pop();
                }
                (Screen::Passphrase, KeyCode::Enter) => {
                    if state.passphrase.len() >= 8 {
                        screen = Screen::Confirm;
                    }
                }

                // Confirmation
                (Screen::Confirm, KeyCode::Char('y') | KeyCode::Char('Y')) => {
                    screen = Screen::Installing;
                    state.install_log.push("Starting installation...".into());

                    // Set up logging channel
                    let (tx, rx) = mpsc::channel();
                    state.log_receiver = Some(rx);

                    // Build install config
                    let device = &state.devices[state.selected_device];
                    let config = InstallConfig {
                        device: PathBuf::from(&device.path),
                        passphrase: state.passphrase.clone(),
                        root_password: state.root_password.clone(),
                        hostname: "mkos".into(),
                        timezone: "UTC".into(),
                        locale: "en_US.UTF-8".into(),
                        keymap: "us".into(),
                        distro: DistroKind::Artix,
                        enable_networking: true,
                        extra_packages: Vec::new(),
                        desktop: Default::default(),
                        swap: Default::default(),
                        audio_enabled: false,
                    };

                    // Spawn install thread
                    thread::spawn(move || {
                        let installer = Installer::new(config);
                        let result = installer.run();

                        match result {
                            Ok(()) => {
                                let _ = tx.send("__COMPLETE__".into());
                            }
                            Err(e) => {
                                let _ = tx.send(format!("__ERROR__:{}", e));
                            }
                        }
                    });
                }
                (Screen::Confirm, KeyCode::Char('n') | KeyCode::Char('N')) => {
                    screen = Screen::DiskSelect;
                }

                // Complete
                (Screen::Complete, KeyCode::Enter) => break,

                // Error
                (Screen::Error(_), KeyCode::Enter) => break,

                _ => {}
            }
        }
    }

    Ok(())
}

fn render(f: &mut Frame, screen: &Screen, state: &InstallerState) {
    let area = f.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let header = Block::default()
        .borders(Borders::ALL)
        .title(" mkOS Installer ");
    f.render_widget(header, chunks[0]);

    // Footer with controls
    let controls = match screen {
        Screen::Welcome => "[Enter] Continue  [q] Quit",
        Screen::DiskSelect => "[↑/↓] Select  [Enter] Continue  [q] Quit",
        Screen::Passphrase => "[Enter] Continue  [Esc] Back",
        Screen::Confirm => "[y] Yes, install  [n] Go back  [q] Quit",
        Screen::Installing => "Installing...",
        Screen::Complete => "[Enter] Finish",
        Screen::Error(_) => "[Enter] Exit",
    };
    let footer = Paragraph::new(controls)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Main content
    let content_area = chunks[1];

    match screen {
        Screen::Welcome => render_welcome(f, content_area),
        Screen::DiskSelect => render_disk_select(f, content_area, state),
        Screen::Passphrase => render_passphrase(f, content_area, state),
        Screen::Confirm => render_confirm(f, content_area, state),
        Screen::Installing => render_installing(f, content_area, state),
        Screen::Complete => render_complete(f, content_area),
        Screen::Error(msg) => render_error(f, content_area, msg),
    }
}

fn render_welcome(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from("Welcome to mkOS").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("This installer will guide you through:"),
        Line::from(""),
        Line::from("  • Disk partitioning (EFI + LUKS2)"),
        Line::from("  • Full disk encryption with Argon2id"),
        Line::from("  • btrfs with subvolumes (@, @home, @snapshots)"),
        Line::from("  • Artix Linux base installation (s6 init)"),
        Line::from("  • EFISTUB boot (no GRUB)"),
        Line::from(""),
        Line::from("Press Enter to continue...").style(Style::default().fg(Color::Green)),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}

fn render_disk_select(f: &mut Frame, area: Rect, state: &InstallerState) {
    let items: Vec<ListItem> = state
        .devices
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let size_gb = d.size_bytes / 1_000_000_000;
            let model = d.model.as_deref().unwrap_or("Unknown");
            let removable = if d.removable { " [removable]" } else { "" };
            let line = format!("{} - {} GB - {}{}", d.path, size_gb, model, removable);

            let style = if i == state.selected_device {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Select Installation Disk "),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

fn render_passphrase(f: &mut Frame, area: Rect, state: &InstallerState) {
    let masked: String = "●".repeat(state.passphrase.len());
    let min_note = if state.passphrase.len() < 8 {
        format!(
            " (minimum 8 characters, {} more needed)",
            8 - state.passphrase.len()
        )
    } else {
        " ✓".into()
    };

    let text = vec![
        Line::from(""),
        Line::from("Enter encryption passphrase:"),
        Line::from(""),
        Line::from(format!("  {}", masked)),
        Line::from(""),
        Line::from(min_note).style(if state.passphrase.len() >= 8 {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        }),
    ];

    let paragraph =
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Encryption "));
    f.render_widget(paragraph, area);
}

fn render_confirm(f: &mut Frame, area: Rect, state: &InstallerState) {
    let device = &state.devices[state.selected_device];
    let size_gb = device.size_bytes / 1_000_000_000;

    let text = vec![
        Line::from(""),
        Line::from("⚠ WARNING").style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("This will DESTROY all data on:"),
        Line::from(""),
        Line::from(format!("  {} ({} GB)", device.path, size_gb))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("Are you sure you want to continue?"),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Confirm Installation "),
    );
    f.render_widget(paragraph, area);
}

fn render_installing(f: &mut Frame, area: Rect, state: &InstallerState) {
    let items: Vec<ListItem> = state
        .install_log
        .iter()
        .map(|s| ListItem::new(s.as_str()))
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" Installing "));
    f.render_widget(list, area);
}

fn render_complete(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from("✓ Installation Complete!").style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from("You can now reboot into your new system."),
        Line::from(""),
        Line::from("Don't forget to:"),
        Line::from("  • Enroll Secure Boot keys in BIOS/UEFI"),
        Line::from("  • Remove the installation media"),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}

fn render_error(f: &mut Frame, area: Rect, msg: &str) {
    let text = vec![
        Line::from(""),
        Line::from("✗ Installation Failed")
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from(msg),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}
