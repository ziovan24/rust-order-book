use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use order_book::App;
use ratatui::backend::CrosstermBackend;
use std::{error::Error, io, time::Duration};


fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new();
    app.add_sample_orders();

    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    mut app: App,
) -> Result<(), Box<dyn Error>> {
    let mut last_update = std::time::Instant::now();
    
    loop {
        terminal.draw(|f| order_book::ui::draw_ui(f, &mut app))?;

        // Auto-update market data every 2 seconds
        if last_update.elapsed() >= Duration::from_secs(2) {
            app.update_market_data();
            app.simulate_real_time_updates();
            last_update = std::time::Instant::now();
        }

        // Non-blocking event reading
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // === QUIT ===
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        return Ok(());
                    }
                    
                    // === TAB NAVIGATION ===
                    KeyCode::Tab => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            app.previous_tab();
                        } else {
                            app.next_tab();
                        }
                    }
                    KeyCode::Right => {
                        app.next_tab();
                    }
                    KeyCode::Left => {
                        app.previous_tab();
                    }
                    
                    // === QUICK TAB ACCESS ===
                    KeyCode::Char('1') => {
                        if app.user_command.is_empty() {
                            app.select_coin_by_index(0);
                        } else {
                            app.selected_tab = 0;
                        }
                    }
                    KeyCode::Char('2') => {
                        if app.user_command.is_empty() {
                            app.select_coin_by_index(1);
                        } else {
                            app.selected_tab = 1;
                        }
                    }
                    KeyCode::Char('3') => {
                        if app.user_command.is_empty() {
                            app.select_coin_by_index(2);
                        } else {
                            app.selected_tab = 2;
                        }
                    }
                    KeyCode::Char('4') => {
                        app.selected_tab = 3;
                    }
                    KeyCode::Char('5') => {
                        app.selected_tab = 4;
                    }
                    KeyCode::Char('6') => {
                        app.selected_tab = 5;
                    }
                    KeyCode::Char('7') => {
                        app.selected_tab = 6;
                    }
                    
                    // === HELP & UTILITIES ===
                    KeyCode::Char('?') | KeyCode::F(1) => {
                        if app.user_command.is_empty() {
                            app.help_mode = !app.help_mode;
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        if app.user_command.is_empty() {
                            app.help_mode = !app.help_mode;
                        }
                    }
                    
                    // === COMMAND MANAGEMENT ===
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        if app.user_command.is_empty() {
                            app.clear_user_command();
                        }
                    }
                    KeyCode::Esc => {
                        app.clear_user_command();
                    }
                    KeyCode::Delete => {
                        app.clear_user_command();
                    }
                    KeyCode::Backspace => {
                        app.remove_user_command();
                    }
                    KeyCode::Enter => {
                        if !app.user_command.is_empty() {
                            app.execute_user_command();
                        } else if app.order_input.active {
                            app.submit_polymarket_order();
                        }
                    }
                    
                    // === MARKET DATA & ORDERS ===
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        if app.user_command.is_empty() {
                            app.add_sample_orders();
                            app.real_time_data.push_back("Sample orders added".to_string());
                        }
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        if app.user_command.is_empty() {
                            app.update_market_data();
                            app.real_time_data.push_back("Market data updated".to_string());
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        if app.user_command.is_empty() {
                            app.refresh_order_book();
                        }
                    }
                    
                    // === ORDER INPUT MODE ===
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        if app.user_command.is_empty() {
                            app.toggle_order_input();
                        }
                    }
                    KeyCode::Char('i') | KeyCode::Char('I') => {
                        if app.user_command.is_empty() {
                            app.toggle_order_input();
                        }
                    }
                    
                    // === ORDER SIDE SELECTION ===
                    KeyCode::Char('b') | KeyCode::Char('B') => {
                        if app.user_command.is_empty() {
                            app.simulate_binance_connection();
                        } else if app.order_input.active {
                            app.order_input.side = order_book::polymarket_orders::PolymarketOrderSide::BUY;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        if app.order_input.active {
                            app.order_input.side = order_book::polymarket_orders::PolymarketOrderSide::SELL;
                        }
                    }
                    
                    // === ORDER TYPE SELECTION ===
                    KeyCode::Char('g') | KeyCode::Char('G') => {
                        if app.order_input.active {
                            app.order_input.order_type = order_book::polymarket_orders::PolymarketOrderType::GTC;
                        }
                    }
                    KeyCode::Char('f') | KeyCode::Char('F') => {
                        if app.order_input.active {
                            app.order_input.order_type = order_book::polymarket_orders::PolymarketOrderType::FOK;
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        if app.order_input.active {
                            app.order_input.order_type = order_book::polymarket_orders::PolymarketOrderType::GTD;
                        }
                    }
                    
                    // === TRADING MODE ===
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        if app.user_command.is_empty() {
                            app.toggle_trading_mode();
                        }
                    }
                    
                    // === COIN SWITCHING ===
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        if app.user_command.is_empty() {
                            app.next_coin();
                        }
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if app.user_command.is_empty() {
                            app.previous_coin();
                        }
                    }
                    
                    // === TIMEFRAME NAVIGATION ===
                    KeyCode::Char('<') | KeyCode::Char(',') => {
                        if app.user_command.is_empty() {
                            app.previous_timeframe();
                        }
                    }
                    KeyCode::Char('>') | KeyCode::Char('.') => {
                        if app.user_command.is_empty() {
                            app.next_timeframe();
                        }
                    }
                    
                    // === AUTO-REFRESH ===
                    KeyCode::Char('l') | KeyCode::Char('L') => {
                        if app.user_command.is_empty() {
                            app.auto_refresh = !app.auto_refresh;
                            app.real_time_data.push_back(format!(
                                "Auto-refresh {}", if app.auto_refresh { "enabled" } else { "disabled" }
                            ));
                        }
                    }
                    
                    // === REAL DATA TOGGLE ===
                    KeyCode::Char('w') | KeyCode::Char('W') => {
                        if app.user_command.is_empty() {
                            app.toggle_real_data();
                        }
                    }
                    
                    // === ORDER FIELD NAVIGATION ===
                    KeyCode::Up => {
                        if app.order_input.active {
                            app.cycle_order_field_up();
                        }
                    }
                    KeyCode::Down => {
                        if app.order_input.active {
                            app.cycle_order_field_down();
                        }
                    }
                    
                    // === FUNCTION KEYS ===
                    KeyCode::F(2) => {
                        app.selected_tab = 0; // Order Book
                    }
                    KeyCode::F(3) => {
                        app.selected_tab = 1; // Trading
                    }
                    KeyCode::F(4) => {
                        app.selected_tab = 2; // Market Data
                    }
                    KeyCode::F(5) => {
                        app.selected_tab = 3; // Orders
                    }
                    KeyCode::F(6) => {
                        app.selected_tab = 4; // Charts
                    }
                    KeyCode::F(7) => {
                        app.selected_tab = 5; // Alerts
                    }
                    KeyCode::F(8) => {
                        app.selected_tab = 6; // Settings
                    }
                    
                    // === SPACE BAR ===
                    KeyCode::Char(' ') => {
                        if app.user_command.is_empty() {
                            app.toggle_order_input();
                        }
                    }
                    
                    // === CHARACTER INPUT ===
                    KeyCode::Char(c) => {
                        if c.is_ascii() && !c.is_control() {
                            app.add_user_command(c);
                        }
                    }
                    
                    _ => {}
                }
            }
        }
    }
}
