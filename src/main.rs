mod app;
mod model;
mod msg;

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::App;
use model::config::AppConfig;
use msg::Msg;

fn main() -> Result<()> {
    // Initialize logging to file (never stdout)
    let log_dir = directories::ProjectDirs::from("", "", "blackbox")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "blackbox.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter("blackbox=info")
        .init();

    tracing::info!("blackbox starting");

    let config = AppConfig::load()?;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, config);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("blackbox error: {e:?}");
    }

    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, config: AppConfig) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let vault_path = config.vault_path();
    let mut app = App::new(config, tx.clone())?;

    // Input thread — reads terminal events and forwards as Msg
    let tx_input = tx.clone();
    thread::spawn(move || {
        loop {
            if let Ok(event) = event::read() {
                let msg = match event {
                    Event::Key(k) => Msg::Key(k),
                    Event::Mouse(m) => Msg::Mouse(m),
                    Event::Resize(w, h) => Msg::Resize(w, h),
                    _ => continue,
                };
                if tx_input.send(msg).is_err() {
                    break;
                }
            }
        }
    });

    // Tick thread — 50ms periodic tick for debounce checks
    let tx_tick = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(50));
            if tx_tick.send(Msg::Tick).is_err() {
                break;
            }
        }
    });

    // File watcher thread — emits FileChanged for create/modify/remove events.
    spawn_file_watcher(vault_path, tx.clone());

    // ── Main event loop ──
    loop {
        // Batch-drain all pending messages
        let first = rx.recv()?;
        app.update(first)?;

        while let Ok(msg) = rx.try_recv() {
            app.update(msg)?;
        }

        if app.should_quit {
            // Final save before exit
            app.update(Msg::SaveAllBuffers)?;
            break;
        }

        terminal.draw(|f| app.view(f))?;
    }

    Ok(())
}

fn spawn_file_watcher(vault_path: PathBuf, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || {
        let tx_watch = tx.clone();
        let mut watcher: RecommendedWatcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                    ) {
                        for path in event.paths {
                            if tx_watch.send(Msg::FileChanged(path)).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("file watcher error: {err}");
                }
            }) {
                Ok(w) => w,
                Err(err) => {
                    tracing::warn!("failed to initialize file watcher: {err}");
                    return;
                }
            };

        if let Err(err) = watcher.watch(&vault_path, RecursiveMode::Recursive) {
            tracing::warn!("failed to watch vault path {}: {err}", vault_path.display());
            return;
        }

        loop {
            thread::park();
        }
    });
}
