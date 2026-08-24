mod app;
mod feed;
mod ui;
mod theme;
mod signal;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture,
        Event, KeyCode, KeyEventKind,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use feed::{FeedMsg, spawn_feeds};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = app::App::new();

    // Unbounded channel — fast, no blocking
    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<FeedMsg>();
    spawn_feeds(tx);

    // 30 FPS render cycle
    let frame_dur = Duration::from_millis(33);

    loop {
        if !app.running {
            break;
        }

        // Tick app state (alert timers etc)
        app.tick();

        // FIRST drain ALL pending messages
        // Yeh CRITICAL hai — draw se pehle sab data lo
        let mut msgs_processed = 0usize;
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    msgs_processed += 1;
                    match msg {
                        FeedMsg::Trade(t) => {
                            app.add_trade(t);
                        }
                        FeedMsg::Book { bids, asks } => {
                            app.update_book(bids, asks);
                        }
                        FeedMsg::Liq(l) => {
                            app.add_liq(l);
                        }
                        FeedMsg::Ticker(s) => {
                            app.update_ticker(s);
                        }
                        FeedMsg::Status { stream, ok } => {
                            match stream {
                                "trade"  => {
                                    app.ws_trade_ok = ok;
                                }
                                "depth"  => {
                                    app.ws_depth_ok = ok;
                                }
                                "liq"    => {
                                    app.ws_liq_ok = ok;
                                }
                                "ticker" => {
                                    app.ws_ticker_ok = ok;
                                }
                                _ => {}
                            }
                        }
                    }
                    // Process max 500 msgs per frame
                    // to avoid UI freeze on burst
                    if msgs_processed >= 500 {
                        break;
                    }
                }
                Err(
                    tokio::sync::mpsc::error::TryRecvError::Empty,
                ) => break,
                Err(
                    tokio::sync::mpsc::error::TryRecvError::Disconnected,
                ) => {
                    app.running = false;
                    break;
                }
            }
        }

        // THEN draw UI with latest data
        terminal.draw(|f| ui::draw(f, &app))?;

        // THEN check keyboard (non-blocking)
        if event::poll(frame_dur)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q')
                        | KeyCode::Char('Q')
                        | KeyCode::Esc => {
                            app.running = false;
                        }
                        KeyCode::Char('r')
                        | KeyCode::Char('R') => {
                            app.reset_session();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("\n══════════════════════════════════");
    println!("  NIFY — Session Ended");
    println!("  Trades      : {}", app.trades.len());
    println!("  Whales      : {}", app.whale_count);
    println!("  Liquidations: {}", app.liquidations.len());
    println!("  Long  Liqs  : {}", app.liq_count_long);
    println!("  Short Liqs  : {}", app.liq_count_short);
    println!(
        "  Total Liq $ : ${:.0}",
        app.total_liq_value
    );
    println!("══════════════════════════════════\n");

    Ok(())
}