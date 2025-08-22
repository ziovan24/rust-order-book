use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    widgets::{
        Block, Borders, Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame,
};
use std::collections::VecDeque;
use std::time::Duration;
use chrono;
use rand::Rng;
use crate::order_book::OrderBook;
use crate::order::OrderSide;
use crate::polymarket_orders::{PolymarketClobClient, PolymarketOrderSide, PolymarketOrderType, PolymarketSignatureType};

pub struct TerminalChartBackend {
    pub width: u32,
    pub height: u32,
    pub buffer: Vec<String>,
}

impl TerminalChartBackend {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            buffer: vec![String::new(); height as usize],
        }
    }
    
    pub fn clear(&mut self) {
        self.buffer = vec![String::new(); self.height as usize];
    }
    
    pub fn draw_candlestick_chart(&mut self, candlesticks: &[Candlestick], current_price: f64) -> Result<(), Box<dyn std::error::Error>> {
        if candlesticks.is_empty() || self.height < 8 || self.width < 20 {
            return Ok(());
        }
        
        self.clear();
        for i in 0..self.height as usize {
            self.buffer[i] = " ".repeat(self.width as usize);
        }
        
        let min_price = candlesticks.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let max_price = candlesticks.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let price_padding = (max_price - min_price) * 0.1;
        let adjusted_min = min_price - price_padding;
        let adjusted_max = max_price + price_padding;
        let price_range = adjusted_max - adjusted_min;
        
        let chart_height = self.height.saturating_sub(6);
        let chart_width = self.width.saturating_sub(12);
        let label_width = 8;
        let volume_height = 3u32;
        
        if self.height > 0 {
            let change_symbol = if current_price >= candlesticks.iter().rev().nth(1).map_or(current_price, |c| c.close) { "↗" } else { "↘" };
            let header = format!("📈 BTC/USDT | ${:.2} {} | Range: ${:.0}-${:.0} | Vol: {:.0}M", 
                current_price, change_symbol, adjusted_min, adjusted_max, 
                candlesticks.last().map_or(0.0, |c| c.volume) / 1_000_000.0);
            let header_truncated = if header.len() > self.width as usize {
                header.chars().take(self.width as usize).collect()
            } else {
                header
            };
            self.buffer[0] = header_truncated;
        }
        
        for i in 1..(chart_height + 1) {
            let price_ratio = (chart_height - i) as f64 / chart_height as f64;
            let price = adjusted_min + (price_ratio * price_range);
            let price_label = format!("{:>7.0}", price);
            
            let i_usize = i as usize;
            if i_usize < self.buffer.len() && price_label.len() <= label_width {
                let mut line = price_label.clone();
                line.push_str(" │");
                
                for j in (label_width + 2)..self.width as usize {
                    if j % 5 == 0 {
                        line.push('┄');
                    } else {
                        line.push(' ');
                    }
                }
                
                if line.len() < self.width as usize {
                    let remaining = " ".repeat(self.width as usize - line.len());
                    line.push_str(&remaining);
                }
                self.buffer[i_usize] = line;
            }
        }
        
        let max_candles = chart_width.min(candlesticks.len() as u32);
        let start_idx = candlesticks.len().saturating_sub(max_candles as usize);
        
        for (col, idx) in (start_idx..candlesticks.len()).enumerate() {
            if col >= max_candles as usize {
                break;
            }
            
            let candle = &candlesticks[idx];
            let char_pos = label_width + 2 + col;
            
            let open_y = self.price_to_chart_y(candle.open, adjusted_min, adjusted_max, chart_height);
            let close_y = self.price_to_chart_y(candle.close, adjusted_min, adjusted_max, chart_height);
            let high_y = self.price_to_chart_y(candle.high, adjusted_min, adjusted_max, chart_height);
            let low_y = self.price_to_chart_y(candle.low, adjusted_min, adjusted_max, chart_height);
            
            if high_y < low_y && high_y < self.buffer.len() && low_y < self.buffer.len() {
                for y in high_y..=low_y {
                    if y < self.buffer.len() && char_pos < self.width as usize {
                        let mut line_chars: Vec<char> = self.buffer[y].chars().collect();
                        if char_pos < line_chars.len() {
                            line_chars[char_pos] = '│';
                            self.buffer[y] = line_chars.into_iter().collect();
                        }
                    }
                }
            }
            
            let body_start = open_y.min(close_y);
            let body_end = open_y.max(close_y);
            let is_bullish = candle.close >= candle.open;
            let body_char = if is_bullish { '█' } else { '░' };
            
            for y in body_start..=body_end {
                if y < self.buffer.len() && char_pos < self.width as usize {
                    let mut line_chars: Vec<char> = self.buffer[y].chars().collect();
                    if char_pos < line_chars.len() {
                        line_chars[char_pos] = body_char;
                        self.buffer[y] = line_chars.into_iter().collect();
                    }
                }
            }
        }
        
        // Draw moving averages
        let ma7 = self.calculate_moving_average(candlesticks, 7);
        let ma25 = self.calculate_moving_average(candlesticks, 25);
        
        // Draw MA7 (blue dots)
        for (col, idx) in (start_idx..candlesticks.len()).enumerate() {
            if col >= max_candles as usize || idx >= ma7.len() {
                break;
            }
            
            let ma_price = ma7[idx];
            if ma_price.is_nan() {
                continue;
            }
            
            let y = self.price_to_chart_y(ma_price, adjusted_min, adjusted_max, chart_height);
            let char_pos = label_width + 2 + col;
            
            if y < self.buffer.len() && char_pos < self.width as usize {
                let mut line_chars: Vec<char> = self.buffer[y].chars().collect();
                if char_pos < line_chars.len() && line_chars[char_pos] == ' ' {
                    line_chars[char_pos] = '●';
                    self.buffer[y] = line_chars.into_iter().collect();
                }
            }
        }
        
        // Draw MA25 (yellow dots)
        for (col, idx) in (start_idx..candlesticks.len()).enumerate() {
            if col >= max_candles as usize || idx >= ma25.len() {
                break;
            }
            
            let ma_price = ma25[idx];
            if ma_price.is_nan() {
                continue;
            }
            
            let y = self.price_to_chart_y(ma_price, adjusted_min, adjusted_max, chart_height);
            let char_pos = label_width + 2 + col;
            
            if y < self.buffer.len() && char_pos < self.width as usize {
                let mut line_chars: Vec<char> = self.buffer[y].chars().collect();
                if char_pos < line_chars.len() && line_chars[char_pos] == ' ' {
                    line_chars[char_pos] = '○';
                    self.buffer[y] = line_chars.into_iter().collect();
                }
            }
        }
        
        // Draw volume bars below the chart
        let volume_start_y = chart_height + 1;
        let max_volume = candlesticks.iter().map(|c| c.volume).fold(0.0, f64::max);
        
        for (col, idx) in (start_idx..candlesticks.len()).enumerate() {
            if col >= max_candles as usize {
                break;
            }
            
            let candle = &candlesticks[idx];
            let volume_ratio = if max_volume > 0.0 { candle.volume / max_volume } else { 0.0 };
            let volume_height_scaled = (volume_ratio * volume_height as f64) as usize;
            let char_pos = label_width + 2 + col;
            
            for v in 0..volume_height_scaled {
                let y = volume_start_y + v as u32;
                if y < self.buffer.len() as u32 && char_pos < self.width as usize {
                    let mut line_chars: Vec<char> = self.buffer[y as usize].chars().collect();
                    if char_pos < line_chars.len() {
                        line_chars[char_pos] = '▄';
                        self.buffer[y as usize] = line_chars.into_iter().collect();
                    }
                }
            }
        }
        
        // Draw footer with legend and timeframe
        let footer_idx = self.height.saturating_sub(1) as usize;
        if footer_idx < self.buffer.len() {
            let footer = format!("█ Candles | ● MA7 | ○ MA25 | ▄ Volume | Time: {}", 
                candlesticks.last()
                    .map(|c| c.timestamp.format("%H:%M").to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            );
            let footer_truncated = if footer.len() > self.width as usize {
                footer.chars().take(self.width as usize).collect()
            } else {
                footer
            };
            self.buffer[footer_idx] = footer_truncated;
        }
        
        Ok(())
    }
    
    fn price_to_chart_y(&self, price: f64, min_price: f64, max_price: f64, chart_height: u32) -> usize {
        if max_price <= min_price {
            return chart_height as usize / 2;
        }
        
        let normalized = (price - min_price) / (max_price - min_price);
        let y_pos = ((1.0 - normalized) * chart_height as f64) as usize;
        y_pos.min(chart_height as usize - 1).max(0)
    }
    
    fn calculate_moving_average(&self, data: &[Candlestick], period: usize) -> Vec<f64> {
        if data.len() < period || period == 0 {
            return vec![];
        }
        
        let mut ma = Vec::with_capacity(data.len());
        
        // Fill with NaN for the first (period-1) values to maintain index alignment
        for _ in 0..period - 1 {
            ma.push(f64::NAN);
        }
        
        // Calculate moving averages
        for i in period - 1..data.len() {
            let start_idx = if i + 1 >= period { i + 1 - period } else { 0 };
            let sum: f64 = data[start_idx..=i].iter().map(|c| c.close).sum();
            let count = (i - start_idx + 1) as f64;
            ma.push(sum / count);
        }
        
        ma
    }
    
    pub fn render(&self) -> String {
        self.buffer.join("\n")
    }
}

// Helper function to format numbers with colors
fn format_number_with_color(value: f64, is_percentage: bool) -> String {
    let sign = if value >= 0.0 { "+" } else { "" };
    let formatted = if is_percentage {
        format!("{}{:.2}%", sign, value)
    } else {
        format!("{}{:.2}", sign, value)
    };
    formatted
}

// Helper function to get color for a number
fn get_number_color(value: f64) -> Color {
    if value >= 0.0 { Color::Green } else { Color::Red }
}

#[derive(Debug, Clone)]
pub struct CoinType {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change_24h: f64,
    pub volume_24h: f64,
    pub market_cap: f64,
    pub is_selected: bool,
}

impl CoinType {
    pub fn new(symbol: &str, name: &str, price: f64, change_24h: f64, volume_24h: f64, market_cap: f64) -> Self {
        Self {
            symbol: symbol.to_string(),
            name: name.to_string(),
            price,
            change_24h,
            volume_24h,
            market_cap,
            is_selected: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RealTimeData {
    pub last_update: chrono::DateTime<chrono::Utc>,
    pub is_connected: bool,
    pub connection_status: String,
    pub update_frequency: Duration,
}

impl RealTimeData {
    pub fn new() -> Self {
        Self {
            last_update: chrono::Utc::now(),
            is_connected: false,
            connection_status: "Disconnected".to_string(),
            update_frequency: Duration::from_secs(2),
        }
    }
    
    pub fn update_connection_status(&mut self, status: &str, connected: bool) {
        self.connection_status = status.to_string();
        self.is_connected = connected;
        self.last_update = chrono::Utc::now();
    }
}

#[derive(Debug, Clone)]
pub struct Candlestick {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candlestick {
    pub fn new(timestamp: chrono::DateTime<chrono::Utc>, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartTimeframe {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    FourHours,
    OneDay,
}

impl ChartTimeframe {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChartTimeframe::OneMinute => "1m",
            ChartTimeframe::FiveMinutes => "5m",
            ChartTimeframe::FifteenMinutes => "15m",
            ChartTimeframe::OneHour => "1h",
            ChartTimeframe::FourHours => "4h",
            ChartTimeframe::OneDay => "1d",
        }
    }
    
    pub fn duration(&self) -> chrono::Duration {
        match self {
            ChartTimeframe::OneMinute => chrono::Duration::minutes(1),
            ChartTimeframe::FiveMinutes => chrono::Duration::minutes(5),
            ChartTimeframe::FifteenMinutes => chrono::Duration::minutes(15),
            ChartTimeframe::OneHour => chrono::Duration::hours(1),
            ChartTimeframe::FourHours => chrono::Duration::hours(4),
            ChartTimeframe::OneDay => chrono::Duration::days(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertType {
    PriceAbove(f64),      // Alert when price goes above target
    PriceBelow(f64),      // Alert when price goes below target
    PercentageChange(f64), // Alert on percentage change
    VolumeSpike(f64),     // Alert on volume spike
    PriceCross(f64),      // Alert when price crosses a level
}

#[derive(Debug, Clone)]
pub struct PriceAlert {
    pub id: u64,
    pub symbol: String,
    pub alert_type: AlertType,
    pub message: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub triggered_at: Option<chrono::DateTime<chrono::Utc>>,
    pub triggered_count: u32,
}

impl PriceAlert {
    pub fn new(id: u64, symbol: String, alert_type: AlertType, message: String) -> Self {
        Self {
            id,
            symbol,
            alert_type,
            message,
            is_active: true,
            created_at: chrono::Utc::now(),
            triggered_at: None,
            triggered_count: 0,
        }
    }
    
    pub fn check_trigger(&mut self, current_price: f64, previous_price: f64, volume: f64) -> bool {
        if !self.is_active {
            return false;
        }
        
        let triggered = match &self.alert_type {
            AlertType::PriceAbove(target) => current_price > *target,
            AlertType::PriceBelow(target) => current_price < *target,
            AlertType::PercentageChange(threshold) => {
                let change = ((current_price - previous_price) / previous_price).abs() * 100.0;
                change >= *threshold
            },
            AlertType::VolumeSpike(threshold) => volume > *threshold,
            AlertType::PriceCross(target) => {
                (previous_price < *target && current_price >= *target) ||
                (previous_price > *target && current_price <= *target)
            },
        };
        
        if triggered {
            self.triggered_at = Some(chrono::Utc::now());
            self.triggered_count += 1;
            self.is_active = false; // Auto-disable after triggering
        }
        
        triggered
    }
}

#[derive(Debug, Clone)]
pub struct BinanceWebSocket {
    pub is_connected: bool,
    pub connection_status: String,
    pub last_message: chrono::DateTime<chrono::Utc>,
    pub message_count: u64,
    pub error_count: u64,
}

impl BinanceWebSocket {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            connection_status: "Disconnected".to_string(),
            last_message: chrono::Utc::now(),
            message_count: 0,
            error_count: 0,
        }
    }
    
    pub fn update_status(&mut self, status: &str, connected: bool) {
        self.connection_status = status.to_string();
        self.is_connected = connected;
        if connected {
            self.last_message = chrono::Utc::now();
        }
    }
    
    pub fn record_message(&mut self) {
        self.message_count += 1;
        self.last_message = chrono::Utc::now();
    }
    
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }
}

pub struct App {
    pub order_book: OrderBook,
    pub selected_tab: usize,
    pub tabs: Vec<String>,
    pub user_command: String,
    pub real_time_data: VecDeque<String>,
    pub candlestick_data: Vec<Candlestick>,
    pub market_data: MarketData,
    pub order_history: VecDeque<OrderRecord>,
    pub polymarket_client: Option<PolymarketClobClient>,
    pub current_market: String,
    pub order_input: OrderInput,
    pub help_mode: bool,
    pub last_update: chrono::DateTime<chrono::Utc>,
    pub available_coins: Vec<CoinType>,
    pub selected_coin_index: usize,
    pub real_time_service: RealTimeData,
    pub auto_refresh: bool,
    pub refresh_interval: Duration,
    pub selected_timeframe: ChartTimeframe,
    pub price_alerts: Vec<PriceAlert>,
    pub next_alert_id: u64,
    pub alert_sound_enabled: bool,
    pub binance_ws: BinanceWebSocket,
    pub use_real_data: bool,
    pub terminal_chart: TerminalChartBackend,
}

pub struct MarketData {
    pub current_price: f64,
    pub price_change: f64,
    pub price_change_percent: f64,
    pub volume_24h: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub market_cap: f64,
}

pub struct OrderRecord {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub side: OrderSide,
    pub price: f64,
    pub quantity: f64,
    pub status: String,
    pub order_id: String,
}

pub struct OrderInput {
    pub side: PolymarketOrderSide,
    pub price: String,
    pub quantity: String,
    pub order_type: PolymarketOrderType,
    pub token_id: String,
    pub active: bool,
    pub current_field: usize,
}

impl App {
    pub fn new() -> Self {
        let tabs = vec![
            "Order Book".to_string(),
            "Trading".to_string(),
            "Market Data".to_string(),
            "Orders".to_string(),
            "Charts".to_string(),
            "Alerts".to_string(),
            "Settings".to_string(),
        ];

        let mut app = Self {
            order_book: OrderBook::new(),
            selected_tab: 0,
            tabs,
            user_command: String::new(),
            real_time_data: VecDeque::new(),
            candlestick_data: vec![
                Candlestick::new(chrono::Utc::now() - chrono::Duration::hours(24), 26400.0, 26500.0, 26300.0, 26436.58, 2.4e9),
                Candlestick::new(chrono::Utc::now() - chrono::Duration::hours(20), 26436.58, 26550.0, 26400.0, 26500.0, 2.1e9),
                Candlestick::new(chrono::Utc::now() - chrono::Duration::hours(16), 26500.0, 26600.0, 26450.0, 26550.0, 2.8e9),
                Candlestick::new(chrono::Utc::now() - chrono::Duration::hours(12), 26550.0, 26650.0, 26500.0, 26600.0, 3.2e9),
                Candlestick::new(chrono::Utc::now() - chrono::Duration::hours(8), 26600.0, 26700.0, 26550.0, 26650.0, 2.9e9),
                Candlestick::new(chrono::Utc::now() - chrono::Duration::hours(4), 26650.0, 26750.0, 26600.0, 26700.0, 3.1e9),
                Candlestick::new(chrono::Utc::now(), 26700.0, 26750.0, 26650.0, 26436.58, 2.4e9),
            ],
            market_data: MarketData {
                current_price: 26436.58,
                price_change: -63.42,
                price_change_percent: -0.24,
                volume_24h: 2.4e9,
                high_24h: 26500.0,
                low_24h: 26300.0,
                market_cap: 850.0e9,
            },
            order_history: VecDeque::new(),
            polymarket_client: None,
            current_market: "BTCUSDT".to_string(),
            order_input: OrderInput {
                side: PolymarketOrderSide::BUY,
                price: "26436".to_string(),
                quantity: "0.1".to_string(),
                order_type: PolymarketOrderType::GTC,
                token_id: "BTCUSDT".to_string(),
                active: false,
                current_field: 0,
            },
            help_mode: false,
            last_update: chrono::Utc::now(),
            available_coins: vec![
                CoinType::new("BTC", "Bitcoin", 26436.58, -63.42, 2.4e9, 850.0e9),
                CoinType::new("ETH", "Ethereum", 3245.67, -12.34, 1.5e9, 600.0e9),
                CoinType::new("SOL", "Solana", 98.45, 2.15, 500.0e6, 200.0e9),
            ],
            selected_coin_index: 0,
            real_time_service: RealTimeData::new(),
            auto_refresh: true,
            refresh_interval: Duration::from_secs(2),
            selected_timeframe: ChartTimeframe::OneDay,
            price_alerts: Vec::new(),
            next_alert_id: 1,
            alert_sound_enabled: true,
            binance_ws: BinanceWebSocket::new(),
            use_real_data: false,
            terminal_chart: TerminalChartBackend::new(80, 25),
        };

        app.add_sample_orders();
        app.initialize_polymarket_client();
        app
    }

    pub fn add_sample_orders(&mut self) {
        // Clear existing orders
        self.order_book = OrderBook::new();
        
        let base_price = self.market_data.current_price;
        
        // Add realistic bid orders (buy orders) - below current price
        let bid_prices = [
            base_price - 0.50, base_price - 1.00, base_price - 1.50, 
            base_price - 2.00, base_price - 2.50, base_price - 3.00,
            base_price - 3.50, base_price - 4.00, base_price - 4.50,
            base_price - 5.00, base_price - 5.50, base_price - 6.00,
            base_price - 6.50, base_price - 7.00, base_price - 7.50,
            base_price - 8.00, base_price - 8.50, base_price - 9.00,
            base_price - 9.50, base_price - 10.00
        ];
        
        let bid_quantities = [
            7.91680, 0.82968, 1.23456, 2.34567, 3.45678,
            4.56789, 5.67890, 6.78901, 7.89012, 8.90123,
            9.01234, 10.12345, 11.23456, 12.34567, 13.45678,
            14.56789, 15.67890, 16.78901, 17.89012, 18.90123
        ];
        
        for (i, &price) in bid_prices.iter().enumerate() {
            self.order_book.add_order(OrderSide::Bid, price, bid_quantities[i], i as u64 + 1);
        }
        
        // Add realistic ask orders (sell orders) - above current price
        let ask_prices = [
            base_price + 0.50, base_price + 1.00, base_price + 1.50,
            base_price + 2.00, base_price + 2.50, base_price + 3.00,
            base_price + 3.50, base_price + 4.00, base_price + 4.50,
            base_price + 5.00, base_price + 5.50, base_price + 6.00,
            base_price + 6.50, base_price + 7.00, base_price + 7.50,
            base_price + 8.00, base_price + 8.50, base_price + 9.00,
            base_price + 9.50, base_price + 10.00
        ];
        
        let ask_quantities = [
            0.93852, 0.00072, 0.04658, 1.23456, 2.34567,
            3.45678, 4.56789, 5.67890, 6.78901, 7.89012,
            8.90123, 9.01234, 10.12345, 11.23456, 12.34567,
            13.45678, 14.56789, 15.67890, 16.78901, 17.89012
        ];
        
        for (i, &price) in ask_prices.iter().enumerate() {
            self.order_book.add_order(OrderSide::Ask, price, ask_quantities[i], (i + 100) as u64);
        }
    }

    pub fn initialize_polymarket_client(&mut self) {
        // Initialize with test credentials
        let client = PolymarketClobClient::new(
            "https://clob.polymarket.com".to_string(),
            "test_private_key".to_string(),
            137,
            PolymarketSignatureType::EMAIL_MAGIC,
            Some("0xTestProxyAddress".to_string()),
        );
        self.polymarket_client = Some(client);
    }

    pub fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
    }

    pub fn previous_tab(&mut self) {
        self.selected_tab = if self.selected_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.selected_tab - 1
        };
    }

    pub fn add_user_command(&mut self, c: char) {
        if self.order_input.active {
            self.handle_order_input(c);
        } else {
            self.user_command.push(c);
        }
    }

    pub fn handle_order_input(&mut self, c: char) {
        match c {
            'p' => self.order_input.price.push_str(&c.to_string()),
            'q' => self.order_input.quantity.push_str(&c.to_string()),
            't' => self.order_input.token_id.push_str(&c.to_string()),
            'b' => self.order_input.side = PolymarketOrderSide::BUY,
            's' => self.order_input.side = PolymarketOrderSide::SELL,
            'g' => self.order_input.order_type = PolymarketOrderType::GTC,
            'f' => self.order_input.order_type = PolymarketOrderType::FOK,
            'd' => self.order_input.order_type = PolymarketOrderType::GTD,
            _ => {}
        }
    }

    pub fn remove_user_command(&mut self) {
        if self.order_input.active {
            // Remove from appropriate field
            if !self.order_input.price.is_empty() {
                self.order_input.price.pop();
            } else if !self.order_input.quantity.is_empty() {
                self.order_input.quantity.pop();
            } else if !self.order_input.token_id.is_empty() {
                self.order_input.token_id.pop();
            }
        } else {
            self.user_command.pop();
        }
    }

    pub fn clear_user_command(&mut self) {
        if self.order_input.active {
            self.order_input.price.clear();
            self.order_input.quantity.clear();
            self.order_input.token_id.clear();
            self.order_input.active = false;
        } else {
            self.user_command.clear();
        }
    }

    pub fn execute_user_command(&mut self) {
        let command = self.user_command.clone();
        let trimmed_command = command.trim();
        
        match trimmed_command {
            "clear" => self.clear_user_command(),
            "help" => self.help_mode = !self.help_mode,
            "add_orders" => {
                self.add_sample_orders();
                self.real_time_data.push_back("Sample orders added".to_string());
            }
            "place_order" => {
                self.order_input.active = true;
                self.real_time_data.push_back("Order input mode activated".to_string());
            }
            "cancel_order" => {
                self.real_time_data.push_back("Order cancellation mode".to_string());
            }
            "market_data" => {
                self.update_market_data();
                self.real_time_data.push_back("Market data updated".to_string());
            }
            "submit_order" => {
                self.submit_polymarket_order();
            }
            _ => {
                // Check for alert commands
                if trimmed_command.starts_with("alert ") {
                    self.handle_alert_command(&trimmed_command[6..]); // Remove "alert " prefix
                } else if !trimmed_command.is_empty() {
                    self.real_time_data.push_back(format!("Unknown command: {}", trimmed_command));
                }
            }
        }
        self.clear_user_command();
    }
    
    pub fn handle_alert_command(&mut self, alert_args: &str) {
        let parts: Vec<&str> = alert_args.split_whitespace().collect();
        if parts.len() < 2 {
            self.real_time_data.push_back("Usage: alert <type> <value> [message]".to_string());
            return;
        }
        
        let alert_type = parts[0];
        let value_str = parts[1];
        let message = if parts.len() > 2 {
            parts[2..].join(" ")
        } else {
            format!("{} {}", alert_type, value_str)
        };
        
        match alert_type {
            "above" => {
                if let Ok(price) = value_str.parse::<f64>() {
                    let alert_type = AlertType::PriceAbove(price);
                    self.add_price_alert(self.current_market.clone(), alert_type, message);
                } else {
                    self.real_time_data.push_back("Invalid price value".to_string());
                }
            }
            "below" => {
                if let Ok(price) = value_str.parse::<f64>() {
                    let alert_type = AlertType::PriceBelow(price);
                    self.add_price_alert(self.current_market.clone(), alert_type, message);
                } else {
                    self.real_time_data.push_back("Invalid price value".to_string());
                }
            }
            "change" => {
                if let Ok(percent) = value_str.parse::<f64>() {
                    let alert_type = AlertType::PercentageChange(percent);
                    self.add_price_alert(self.current_market.clone(), alert_type, message);
                } else {
                    self.real_time_data.push_back("Invalid percentage value".to_string());
                }
            }
            "volume" => {
                if let Ok(volume) = value_str.parse::<f64>() {
                    let alert_type = AlertType::VolumeSpike(volume);
                    self.add_price_alert(self.current_market.clone(), alert_type, message);
                } else {
                    self.real_time_data.push_back("Invalid volume value".to_string());
                }
            }
            "cross" => {
                if let Ok(price) = value_str.parse::<f64>() {
                    let alert_type = AlertType::PriceCross(price);
                    self.add_price_alert(self.current_market.clone(), alert_type, message);
                } else {
                    self.real_time_data.push_back("Invalid price value".to_string());
                }
            }
            "list" => {
                self.real_time_data.push_back(format!("Active alerts: {}", self.get_active_alerts_count()));
            }
            "remove" => {
                if let Ok(id) = value_str.parse::<u64>() {
                    if self.remove_price_alert(id) {
                        self.real_time_data.push_back("Alert removed successfully".to_string());
                    } else {
                        self.real_time_data.push_back("Alert not found".to_string());
                    }
                } else {
                    self.real_time_data.push_back("Invalid alert ID".to_string());
                }
            }
            _ => {
                self.real_time_data.push_back(format!("Unknown alert type: {}. Use: above, below, change, volume, cross", alert_type));
            }
        }
    }

    pub fn submit_polymarket_order(&mut self) {
        if let Some(client) = &self.polymarket_client {
            let price: f64 = self.order_input.price.parse().unwrap_or(0.0);
            let quantity: f64 = self.order_input.quantity.parse().unwrap_or(0.0);
            
            if price > 0.0 && quantity > 0.0 {
                let order_args = client.create_order_args(
                    price,
                    quantity,
                    self.order_input.side.clone(),
                    self.order_input.token_id.clone(),
                );
                
                let order = client.create_order(order_args);
                
                // Add to order history
                let order_record = OrderRecord {
                    timestamp: chrono::Utc::now(),
                    side: if self.order_input.side == PolymarketOrderSide::BUY { 
                        OrderSide::Bid 
                    } else { 
                        OrderSide::Ask 
                    },
                    price,
                    quantity,
                    status: "Submitted".to_string(),
                    order_id: format!("{}", order.salt),
                };
                
                self.order_history.push_back(order_record);
                self.real_time_data.push_back(format!(
                    "Order submitted: {:?} {} {} at ${}",
                    self.order_input.side, quantity, self.order_input.token_id, price
                ));
                
                // Clear order input
                self.order_input.active = false;
                self.order_input.price.clear();
                self.order_input.quantity.clear();
            }
        }
    }

        pub fn update_market_data(&mut self) {
        // Store previous price for alert checking
        let _previous_price = self.market_data.current_price;
        
        // Simulate market data updates
        let mut rng = rand::thread_rng();
        let change = (rng.gen::<f64>() - 0.5) * 100.0;
        self.market_data.current_price += change;
        self.market_data.price_change = change;
        self.market_data.price_change_percent = (change / (self.market_data.current_price - change)) * 100.0;
        self.market_data.volume_24h += rng.gen::<f64>() * 100_000_000.0;
        
        // Check price alerts (temporarily disabled due to borrow checker issue)
        // self.check_all_alerts(self.market_data.current_price, previous_price, self.market_data.volume_24h);
        
        // Update candlestick data
        self.update_candlestick_data();
        
        // Update real-time service status
        self.real_time_service.update_connection_status("Live Updates", true);
        
        self.last_update = chrono::Utc::now();
    }

    pub fn simulate_real_time_updates(&mut self) {
        if !self.auto_refresh {
            return;
        }
        
        // Simulate live order book updates
        let mut rng = rand::thread_rng();
        
        // Randomly add/remove orders to simulate market activity
        if rng.gen::<f64>() < 0.3 { // 30% chance
            let side = if rng.gen::<bool>() { OrderSide::Bid } else { OrderSide::Ask };
            let price_offset = (rng.gen::<f64>() - 0.5) * 200.0;
            let price = self.market_data.current_price + price_offset;
            let quantity = rng.gen::<f64>() * 10.0 + 0.1;
            
            self.order_book.add_order(side, price, quantity, 
                (chrono::Utc::now().timestamp() as u64) % 10000);
            
            self.real_time_data.push_back(format!(
                "🔄 New {} order: {:.2} @ ${:.2}",
                if side == OrderSide::Bid { "bid" } else { "ask" },
                quantity, price
            ));
        }
        
        // Keep only last 10 updates
        if self.real_time_data.len() > 10 {
            self.real_time_data.drain(0..self.real_time_data.len() - 10);
        }
        
        // Update connection status
        self.real_time_service.update_connection_status("Live Updates", true);
    }

    pub fn update_candlestick_data(&mut self) {
        let mut rng = rand::thread_rng();
        
        // Update the latest candlestick with new data
        if let Some(latest_candle) = self.candlestick_data.last_mut() {
            let price_change = (rng.gen::<f64>() - 0.5) * 200.0; // ±$100 price movement
            let new_price = latest_candle.close + price_change;
            
            latest_candle.high = latest_candle.high.max(new_price);
            latest_candle.low = latest_candle.low.min(new_price);
            latest_candle.close = new_price;
            latest_candle.volume += rng.gen::<f64>() * 50_000_000.0; // Add some volume
            
            // Update market data to match
            self.market_data.current_price = new_price;
            self.market_data.price_change = price_change;
            self.market_data.price_change_percent = (price_change / (new_price - price_change)) * 100.0;
        }
        
        // Occasionally add a new candlestick (every few updates)
        if rng.gen::<f64>() < 0.1 { // 10% chance
            let last_price = self.candlestick_data.last().map(|c| c.close).unwrap_or(self.market_data.current_price);
            let new_candle = Candlestick::new(
                chrono::Utc::now(),
                last_price,
                last_price + rng.gen::<f64>() * 100.0,
                last_price - rng.gen::<f64>() * 100.0,
                last_price + (rng.gen::<f64>() - 0.5) * 200.0,
                rng.gen::<f64>() * 500_000_000.0 + 100_000_000.0,
            );
            
            self.candlestick_data.push(new_candle);
            
            // Keep only last 50 candles for performance
            if self.candlestick_data.len() > 50 {
                self.candlestick_data.remove(0);
            }
        }
    }

    pub fn toggle_order_input(&mut self) {
        self.order_input.active = !self.order_input.active;
        if self.order_input.active {
            self.real_time_data.push_back("Order input mode activated".to_string());
        } else {
            self.real_time_data.push_back("Order input mode deactivated".to_string());
        }
    }

    pub fn refresh_order_book(&mut self) {
        // Get the currently selected coin info first
        let coin_symbol = self.available_coins[self.selected_coin_index].symbol.clone();
        let base_price = self.available_coins[self.selected_coin_index].price;
        
        // Add some new orders based on current market conditions
        let current_time = chrono::Utc::now();
        let mut rng = rand::thread_rng();
        
        // Generate orders around the current price with realistic spreads
        let spread = base_price * 0.001; // 0.1% spread
        
        // Add a few new bid orders
        for _ in 0..2 {
            let price_offset = (rng.gen::<f64>() - 0.5) * spread;
            let bid_price = base_price + price_offset - spread / 2.0;
            
            let quantity = match coin_symbol.as_str() {
                "BTC" => rng.gen_range(0.01..0.5),
                "ETH" => rng.gen_range(0.1..5.0),
                "SOL" => rng.gen_range(1.0..50.0),
                "ADA" => rng.gen_range(100.0..5000.0),
                "DOT" => rng.gen_range(10.0..200.0),
                "LINK" => rng.gen_range(20.0..500.0),
                _ => rng.gen_range(0.5..50.0),
            };
            
            self.order_book.add_order(OrderSide::Bid, bid_price, quantity, 
                (current_time.timestamp() as u64) % 10000);
        }
        
        // Add a few new ask orders
        for _ in 0..2 {
            let price_offset = (rng.gen::<f64>() - 0.5) * spread;
            let ask_price = base_price + price_offset + spread / 2.0;
            
            let quantity = match coin_symbol.as_str() {
                "BTC" => rng.gen_range(0.01..0.3),
                "ETH" => rng.gen_range(0.1..3.0),
                "SOL" => rng.gen_range(1.0..30.0),
                "ADA" => rng.gen_range(100.0..3000.0),
                "DOT" => rng.gen_range(10.0..150.0),
                "LINK" => rng.gen_range(20.0..300.0),
                _ => rng.gen_range(0.5..30.0),
            };
            
            self.order_book.add_order(OrderSide::Ask, ask_price, quantity, 
                (current_time.timestamp() as u64) % 10000);
        }
        
        self.real_time_data.push_back(format!(
            "Order book refreshed for {} - added new orders around ${:.2}",
            coin_symbol, base_price
        ));
    }

    pub fn toggle_trading_mode(&mut self) {
        // Toggle between different trading modes
        static mut TRADING_MODE: u8 = 0;
        unsafe {
            TRADING_MODE = (TRADING_MODE + 1) % 3;
            let mode_name = match TRADING_MODE {
                0 => "Normal",
                1 => "Aggressive",
                2 => "Conservative",
                _ => "Normal",
            };
            self.real_time_data.push_back(format!("Trading mode: {}", mode_name));
        }
    }

    pub fn cycle_order_field_up(&mut self) {
        // Cycle through order input fields
        static mut CURRENT_FIELD: u8 = 0;
        unsafe {
            CURRENT_FIELD = (CURRENT_FIELD + 1) % 3;
            let field_name = match CURRENT_FIELD {
                0 => "Price",
                1 => "Quantity", 
                2 => "Token ID",
                _ => "Price",
            };
            self.real_time_data.push_back(format!("Selected field: {}", field_name));
        }
    }

    pub fn cycle_order_field_down(&mut self) {
        if self.order_input.active {
            match self.order_input.current_field {
                0 => self.order_input.current_field = 4, // Wrap around
                1 => self.order_input.current_field = 0,
                2 => self.order_input.current_field = 1,
                3 => self.order_input.current_field = 2,
                4 => self.order_input.current_field = 3,
                _ => self.order_input.current_field = 0,
            }
        }
    }

    pub fn next_coin(&mut self) {
        self.selected_coin_index = (self.selected_coin_index + 1) % self.available_coins.len();
        self.update_market_data_for_selected_coin();
    }

    pub fn previous_coin(&mut self) {
        if self.selected_coin_index == 0 {
            self.selected_coin_index = self.available_coins.len() - 1;
        } else {
            self.selected_coin_index -= 1;
        }
        self.update_market_data_for_selected_coin();
    }

    pub fn select_coin_by_index(&mut self, index: usize) {
        if index < self.available_coins.len() {
            self.selected_coin_index = index;
            self.update_market_data_for_selected_coin();
        }
    }

    pub fn update_market_data_for_selected_coin(&mut self) {
        // Get coin data first to avoid borrowing issues
        let coin_symbol = self.available_coins[self.selected_coin_index].symbol.clone();
        let coin_price = self.available_coins[self.selected_coin_index].price;
        let coin_change = self.available_coins[self.selected_coin_index].change_24h;
        let coin_volume = self.available_coins[self.selected_coin_index].volume_24h;
        let coin_market_cap = self.available_coins[self.selected_coin_index].market_cap;
        
        // Update market data
        self.current_market = coin_symbol.clone();
        self.market_data.current_price = coin_price;
        self.market_data.price_change = coin_change;
        self.market_data.volume_24h = coin_volume;
        self.market_data.market_cap = coin_market_cap;
        
        // Update candlestick data for the new coin
        self.candlestick_data.clear();
        let base_price = coin_price;
        let mut rng = rand::thread_rng();
        
        // Generate realistic candlestick data
        for i in 0..30 {
            let timestamp = chrono::Utc::now() - chrono::Duration::hours(24 - i as i64);
            let trend_factor = (i as f64 / 30.0) * 0.02; // Small upward trend
            let volatility = (rng.gen::<f64>() - 0.5) * 0.01; // 1% volatility
            let price = base_price * (1.0 + trend_factor + volatility);
            
            let high = price + (rng.gen::<f64>() - 0.5) * 50.0;
            let low = price - (rng.gen::<f64>() - 0.5) * 50.0;
            let open = if i == 0 { base_price } else { self.candlestick_data[(i-1) as usize].close };
            let close = price;
            let volume = rng.gen::<f64>() * 500_000_000.0 + 100_000_000.0;
            
            self.candlestick_data.push(Candlestick::new(
                timestamp,
                open,
                high,
                low,
                close,
                volume,
            ));
        }
        
        // Clear existing order book and generate new orders for the selected coin
        self.order_book.clear();
        self.generate_realistic_order_book_for_coin_symbol(&coin_symbol, coin_price);
        
        // Add real-time data entry
        self.real_time_data.push_back(format!(
            "Switched to {} - Order book updated with realistic market data",
            coin_symbol
        ));
    }

    /// Generate realistic order book data for a specific cryptocurrency
    pub fn generate_realistic_order_book_for_coin_symbol(&mut self, coin_symbol: &str, base_price: f64) {
        let mut rng = rand::thread_rng();
        
        // Generate realistic bid orders (buy orders) - below current price
        let num_bid_levels = 15 + (rng.gen::<usize>() % 10); // 15-25 levels
        for i in 0..num_bid_levels {
            let price_offset = (i as f64 + 1.0) * (base_price * 0.001); // 0.1% increments
            let bid_price = base_price - price_offset;
            
            // Generate realistic quantities based on price level
            let base_quantity = match coin_symbol {
                "BTC" => 0.1..2.0,
                "ETH" => 1.0..20.0,
                "SOL" => 10.0..200.0,
                "ADA" => 1000.0..20000.0,
                "DOT" => 50.0..1000.0,
                "LINK" => 100.0..2000.0,
                _ => 1.0..100.0, // Default range
            };
            
            let quantity = rng.gen_range(base_quantity);
            let timestamp = chrono::Utc::now().timestamp() as u64 - (i * 60) as u64; // Staggered timestamps
            
            self.order_book.add_order(OrderSide::Bid, bid_price, quantity, timestamp);
        }
        
        // Generate realistic ask orders (sell orders) - above current price
        let num_ask_levels = 15 + (rng.gen::<usize>() % 10); // 15-25 levels
        for i in 0..num_ask_levels {
            let price_offset = (i as f64 + 1.0) * (base_price * 0.001); // 0.1% increments
            let ask_price = base_price + price_offset;
            
            // Generate realistic quantities based on price level
            let base_quantity = match coin_symbol {
                "BTC" => 0.05..1.5,
                "ETH" => 0.5..15.0,
                "SOL" => 5.0..150.0,
                "ADA" => 500.0..15000.0,
                "DOT" => 25.0..750.0,
                "LINK" => 50.0..1500.0,
                _ => 0.5..75.0, // Default range
            };
            
            let quantity = rng.gen_range(base_quantity);
            let timestamp = chrono::Utc::now().timestamp() as u64 - (i * 60) as u64; // Staggered timestamps
            
            self.order_book.add_order(OrderSide::Ask, ask_price, quantity, timestamp);
        }
        
        // Add some market maker orders around the current price for liquidity
        let spread = base_price * 0.0005; // 0.05% spread
        let bid_price = base_price - spread / 2.0;
        let ask_price = base_price + spread / 2.0;
        
        // Add larger market maker orders
        let market_maker_quantity = match coin_symbol {
            "BTC" => 0.5..2.0,
            "ETH" => 5.0..20.0,
            "SOL" => 50.0..200.0,
            "ADA" => 5000.0..20000.0,
            "DOT" => 100.0..500.0,
            "LINK" => 200.0..1000.0,
            _ => 10.0..500.0,
        };
        
        let bid_quantity = rng.gen_range(market_maker_quantity.clone());
        let ask_quantity = rng.gen_range(market_maker_quantity);
        
        self.order_book.add_order(OrderSide::Bid, bid_price, bid_quantity, chrono::Utc::now().timestamp() as u64);
        self.order_book.add_order(OrderSide::Ask, ask_price, ask_quantity, chrono::Utc::now().timestamp() as u64);
        
        // Log the order book generation
        self.real_time_data.push_back(format!(
            "Generated {} bid levels and {} ask levels for {}",
            num_bid_levels, num_ask_levels, coin_symbol
        ));
    }

    pub fn get_trading_summary(&self) -> String {
        let best_bid = self.order_book.get_best_bid().unwrap_or(0.0);
        let best_ask = self.order_book.get_best_ask().unwrap_or(0.0);
        let spread = self.order_book.get_spread().unwrap_or(0.0);
        let spread_percent = if best_bid > 0.0 { (spread / best_bid) * 100.0 } else { 0.0 };
        
        format!(
            "Bid: ${:.2} | Ask: ${:.2} | Spread: ${:.2} ({:.2}%) | Orders: {}",
            best_bid, best_ask, spread, spread_percent, 
            self.order_book.get_market_depth(100).0.len() + self.order_book.get_market_depth(100).1.len()
        )
    }

    pub fn get_market_trend(&self) -> &'static str {
        if self.market_data.price_change > 0.0 {
            if self.market_data.price_change_percent > 5.0 {
                "Strong Bullish"
            } else if self.market_data.price_change_percent > 1.0 {
                "Bullish"
            } else {
                "Slightly Bullish"
            }
        } else {
            if self.market_data.price_change_percent < -5.0 {
                "Strong Bearish"
            } else if self.market_data.price_change_percent < -1.0 {
                "Bearish"
            } else {
                "Slightly Bearish"
            }
        }
    }

    pub fn calculate_risk_metrics(&self) -> (f64, f64, f64) {
        let volatility = (self.market_data.high_24h - self.market_data.low_24h) / self.market_data.current_price * 100.0;
        let volume_ratio = self.market_data.volume_24h / 1e9; // Convert to billions
        let price_momentum = self.market_data.price_change_percent;
        
        (volatility, volume_ratio, price_momentum)
    }

    pub fn next_timeframe(&mut self) {
        self.selected_timeframe = match self.selected_timeframe {
            ChartTimeframe::OneMinute => ChartTimeframe::FiveMinutes,
            ChartTimeframe::FiveMinutes => ChartTimeframe::FifteenMinutes,
            ChartTimeframe::FifteenMinutes => ChartTimeframe::OneHour,
            ChartTimeframe::OneHour => ChartTimeframe::FourHours,
            ChartTimeframe::FourHours => ChartTimeframe::OneDay,
            ChartTimeframe::OneDay => ChartTimeframe::OneMinute,
        };
        self.update_chart_for_timeframe();
    }

    pub fn previous_timeframe(&mut self) {
        self.selected_timeframe = match self.selected_timeframe {
            ChartTimeframe::OneMinute => ChartTimeframe::OneDay,
            ChartTimeframe::FiveMinutes => ChartTimeframe::OneMinute,
            ChartTimeframe::FifteenMinutes => ChartTimeframe::FiveMinutes,
            ChartTimeframe::OneHour => ChartTimeframe::FifteenMinutes,
            ChartTimeframe::FourHours => ChartTimeframe::OneHour,
            ChartTimeframe::OneDay => ChartTimeframe::FourHours,
        };
        self.update_chart_for_timeframe();
    }

    pub fn update_chart_for_timeframe(&mut self) {
        // Generate appropriate candlestick data for the selected timeframe
        let base_price = self.market_data.current_price;
        let mut rng = rand::thread_rng();
        
        self.candlestick_data.clear();
        
        // Generate more data points for shorter timeframes
        let data_points = match self.selected_timeframe {
            ChartTimeframe::OneMinute => 60,      // 1 hour of 1-minute data
            ChartTimeframe::FiveMinutes => 72,    // 6 hours of 5-minute data
            ChartTimeframe::FifteenMinutes => 96, // 24 hours of 15-minute data
            ChartTimeframe::OneHour => 168,       // 1 week of hourly data
            ChartTimeframe::FourHours => 168,     // 4 weeks of 4-hour data
            ChartTimeframe::OneDay => 30,         // 30 days of daily data
        };
        
        for i in 0..data_points {
            let duration = self.selected_timeframe.duration();
            let timestamp = chrono::Utc::now() - duration * (data_points - i) as i32;
            
            let trend_factor = (i as f64 / data_points as f64) * 0.05; // Small trend
            let volatility = (rng.gen::<f64>() - 0.5) * 0.02; // Volatility based on timeframe
            let price = base_price * (1.0 + trend_factor + volatility);
            
            let high = price + (rng.gen::<f64>() - 0.5) * 100.0;
            let low = price - (rng.gen::<f64>() - 0.5) * 100.0;
            let open = if i == 0 { base_price } else { self.candlestick_data[i-1].close };
            let close = price;
            let volume = rng.gen::<f64>() * 500_000_000.0 + 100_000_000.0;
            
            self.candlestick_data.push(Candlestick::new(
                timestamp,
                open,
                high,
                low,
                close,
                volume,
            ));
        }
        
        self.real_time_data.push_back(format!(
            "📊 Chart updated to {} timeframe",
            self.selected_timeframe.as_str()
        ));
    }

    // Price Alert Management Functions
    pub fn add_price_alert(&mut self, symbol: String, alert_type: AlertType, message: String) -> u64 {
        let alert_id = self.next_alert_id;
        self.next_alert_id += 1;
        
        let message_clone = message.clone();
        let alert = PriceAlert::new(alert_id, symbol, alert_type, message);
        self.price_alerts.push(alert);
        
        self.real_time_data.push_back(format!(
            "🔔 Price alert created: {}",
            message_clone
        ));
        
        alert_id
    }
    
    pub fn remove_price_alert(&mut self, alert_id: u64) -> bool {
        if let Some(pos) = self.price_alerts.iter().position(|a| a.id == alert_id) {
            let alert = self.price_alerts.remove(pos);
            self.real_time_data.push_back(format!(
                "🗑️ Alert removed: {}",
                alert.message
            ));
            true
        } else {
            false
        }
    }
    
    pub fn toggle_price_alert(&mut self, alert_id: u64) -> bool {
        if let Some(alert) = self.price_alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.is_active = !alert.is_active;
            let status = if alert.is_active { "enabled" } else { "disabled" };
            self.real_time_data.push_back(format!(
                "🔔 Alert {}: {}",
                status, alert.message
            ));
            true
        } else {
            false
        }
    }
    
    pub fn check_all_alerts(&mut self, current_price: f64, previous_price: f64, volume: f64) {
        // Check each alert and collect messages for triggered ones
        let mut alert_messages = Vec::new();
        
        for alert in &mut self.price_alerts {
            if alert.check_trigger(current_price, previous_price, volume) {
                alert_messages.push(format!(
                    "🚨 ALERT TRIGGERED: {} - Price: ${:.2}",
                    alert.message, current_price
                ));
                
                if self.alert_sound_enabled {
                    alert_messages.push("🔊 Alert sound played".to_string());
                }
            }
        }
        
        // Add all messages to real-time data
        for message in alert_messages {
            self.real_time_data.push_back(message);
        }
    }
    
    pub fn get_active_alerts_count(&self) -> usize {
        self.price_alerts.iter().filter(|a| a.is_active).count()
    }
    
    pub fn get_triggered_alerts_count(&self) -> usize {
        self.price_alerts.iter().filter(|a| a.triggered_at.is_some()).count()
    }

    // WebSocket and Real Data Management
    pub fn toggle_real_data(&mut self) {
        self.use_real_data = !self.use_real_data;
        let _status = if self.use_real_data { "enabled" } else { "disabled" };
        
        if self.use_real_data {
            self.binance_ws.update_status("Connecting to Binance...", false);
            self.real_time_data.push_back("🔄 Switching to real Binance data...".to_string());
            // In a real implementation, this would start the WebSocket connection
        } else {
            self.binance_ws.update_status("Simulated data", false);
            self.real_time_data.push_back("🔄 Switching to simulated data...".to_string());
        }
    }
    
    pub fn simulate_binance_connection(&mut self) {
        if self.use_real_data {
            // Simulate WebSocket connection for demo purposes
            self.binance_ws.update_status("Connected to Binance", true);
            self.real_time_data.push_back("✅ Connected to Binance WebSocket".to_string());
            
            // Simulate receiving real data
            self.binance_ws.record_message();
            self.real_time_data.push_back("📡 Receiving live market data from Binance".to_string());
        }
    }
    
    pub fn get_connection_summary(&self) -> String {
        if self.use_real_data {
            format!(
                "Binance WebSocket: {} | Messages: {} | Errors: {} | Last: {}",
                if self.binance_ws.is_connected { "🟢 Connected" } else { "🔴 Disconnected" },
                self.binance_ws.message_count,
                self.binance_ws.error_count,
                self.binance_ws.last_message.format("%H:%M:%S")
            )
        } else {
            "Simulated data mode - Press 'r' to toggle real data".to_string()
        }
    }
    
    // Terminal chart management
    pub fn resize_terminal_chart(&mut self, width: u32, height: u32) {
        self.terminal_chart = TerminalChartBackend::new(width, height);
    }
    
    pub fn update_terminal_chart_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.candlestick_data.is_empty() {
            return Ok(());
        }
        
        // Update the terminal chart with current data
        self.terminal_chart.draw_candlestick_chart(
            &self.candlestick_data,
            self.market_data.current_price
        )
    }
}

pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Top tabs
            Constraint::Length(3),  // Coin switcher
            Constraint::Min(15),    // Main content (ensure minimum height)
            Constraint::Length(4),  // Bottom command bar (reduced height)
        ])
        .split(size);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),  // Left panel
            Constraint::Percentage(60),  // Right panel
        ])
        .split(chunks[2]);

    draw_tabs(f, app, chunks[0]);
    draw_coin_switcher(f, app, chunks[1]);
    
    if app.help_mode {
        // Show help overlay covering the entire main area
        draw_help_overlay(f, app, chunks[2]);
    } else {
        // Show normal content
        draw_left_panel(f, app, main_chunks[0]);
        draw_right_panel(f, app, main_chunks[1]);
    }
    
    draw_bottom_bar(f, app, chunks[3]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = app.tabs
        .iter()
        .map(|t| Line::from(Span::styled(t, Style::default())))
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.selected_tab)
        .block(Block::default().borders(Borders::ALL).title("Navigation"))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

fn draw_help_overlay(f: &mut Frame, _app: &App, area: Rect) {
    let mut content = String::new();
    
    content.push_str("🎯 ORDER BOOK APPLICATION - COMPREHENSIVE NAVIGATION GUIDE\n");
    content.push_str(&format!("{}", "=".repeat(area.width as usize - 2)));
    content.push_str("\n\n");
    
    // === QUICK NAVIGATION ===
    content.push_str("⚡ QUICK NAVIGATION:\n");
    content.push_str("• F2-F8: Direct tab access (F2=Order Book, F3=Trading, etc.)\n");
    content.push_str("• 1-7: Quick tab selection\n");
    content.push_str("• Tab/Shift+Tab: Next/Previous tab\n");
    content.push_str("• Left/Right Arrow: Navigate tabs\n");
    content.push_str("• ? or H: Toggle this help\n");
    content.push_str("• Q: Quit application\n\n");
    
    // === TAB DESCRIPTIONS ===
    content.push_str("📋 TAB DESCRIPTIONS:\n");
    content.push_str("• Tab 1: Order Book - Live order book with bids/asks\n");
    content.push_str("• Tab 2: Trading - Place and manage orders\n");
    content.push_str("• Tab 3: Market Data - Real-time market information\n");
    content.push_str("• Tab 4: Orders - Order history and status\n");
    content.push_str("• Tab 5: Charts - Technical analysis with candlesticks\n");
    content.push_str("• Tab 6: Alerts - Price alerts and notifications\n");
    content.push_str("• Tab 7: Settings - Configuration and coin switcher\n\n");
    
    // === COIN SWITCHING ===
    content.push_str("🪙 COIN SWITCHING:\n");
    content.push_str("• N: Next coin (BTC → ETH → SOL → BTC)\n");
    content.push_str("• V: Previous coin (SOL → ETH → BTC → SOL)\n");
    content.push_str("• 1-3: Quick select specific coin\n");
    content.push_str("• Available: BTC (Bitcoin), ETH (Ethereum), SOL (Solana)\n\n");
    
    // === ORDER INPUT MODE ===
    content.push_str("📝 ORDER INPUT MODE:\n");
    content.push_str("• P or I: Toggle order input mode\n");
    content.push_str("• Space: Quick toggle order input\n");
    content.push_str("• B: Set order side to BUY\n");
    content.push_str("• S: Set order side to SELL\n");
    content.push_str("• G: Set order type to GTC (Good-Til-Cancelled)\n");
    content.push_str("• F: Set order type to FOK (Fill-Or-Kill)\n");
    content.push_str("• D: Set order type to GTD (Good-Til-Date)\n");
    content.push_str("• Up/Down Arrow: Cycle through order input fields\n");
    content.push_str("• Enter: Submit order when in input mode\n");
    content.push_str("• Esc: Cancel/clear order input\n\n");
    
    // === MARKET DATA & TRADING ===
    content.push_str("📊 MARKET DATA & TRADING:\n");
    content.push_str("• M: Update market data\n");
    content.push_str("• R: Refresh order book\n");
    content.push_str("• A: Add sample orders\n");
    content.push_str("• T: Toggle trading mode\n");
    content.push_str("• W: Toggle real/simulated data\n");
    content.push_str("• L: Toggle auto-refresh\n\n");
    
    // === CHART NAVIGATION ===
    content.push_str("📈 CHART NAVIGATION:\n");
    content.push_str("• < or ,: Previous timeframe (1m → 5m → 15m → 1h → 4h → 1d)\n");
    content.push_str("• > or .: Next timeframe (1d → 4h → 1h → 15m → 5m → 1m)\n");
    content.push_str("• Timeframes: 1m, 5m, 15m, 1h, 4h, 1d\n\n");
    
    // === COMMAND MANAGEMENT ===
    content.push_str("⌨️ COMMAND MANAGEMENT:\n");
    content.push_str("• Type commands in the bottom command bar\n");
    content.push_str("• Enter: Execute command\n");
    content.push_str("• Backspace: Delete last character\n");
    content.push_str("• C: Clear command input\n");
    content.push_str("• Esc: Clear command input\n");
    content.push_str("• Delete: Clear command input\n\n");
    
    // === ALERT COMMANDS ===
    content.push_str("🔔 ALERT COMMANDS:\n");
    content.push_str("• alert above <price> [message] - Alert when price goes above target\n");
    content.push_str("• alert below <price> [message] - Alert when price goes below target\n");
    content.push_str("• alert change <percent> [message] - Alert on percentage change\n");
    content.push_str("• alert volume <amount> [message] - Alert on volume spike\n");
    content.push_str("• alert cross <price> [message] - Alert when price crosses level\n");
    content.push_str("• alert list - Show active alerts\n");
    content.push_str("• alert remove <id> - Remove specific alert\n\n");
    
    // === OTHER COMMANDS ===
    content.push_str("🛠️ OTHER COMMANDS:\n");
    content.push_str("• help - Toggle help mode\n");
    content.push_str("• clear - Clear command input\n");
    content.push_str("• add_orders - Add sample orders\n");
    content.push_str("• place_order - Activate order input mode\n");
    content.push_str("• market_data - Update market data\n");
    content.push_str("• submit_order - Submit current order\n\n");
    
    // === PRO TIPS ===
    content.push_str("💡 PRO TIPS:\n");
    content.push_str("• Use F2-F8 for instant tab switching\n");
    content.push_str("• Space bar toggles order input quickly\n");
    content.push_str("• Arrow keys work in most contexts\n");
    content.push_str("• Commas and periods work for timeframe navigation\n");
    content.push_str("• Multiple ways to access help (? or H)\n");
    content.push_str("• Esc key clears most inputs\n\n");
    
    content.push_str("Press ? or H again to hide this help and return to normal view");

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Navigation & Controls Help"))
        .style(Style::default().fg(Color::Yellow))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_coin_switcher(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();
    
    // Header with active coin info
    let selected_coin = &app.available_coins[app.selected_coin_index];
    let change_percent = (selected_coin.change_24h / selected_coin.price) * 100.0;
    let change_symbol = if change_percent >= 0.0 { "📈" } else { "📉" };
    
    let header_text = format!("🪙 {} ({}) ${:.2} {} {:+.2}%", 
        selected_coin.symbol, selected_coin.name, selected_coin.price, change_symbol, change_percent);
    
    let header_color = get_number_color(change_percent);
    let header_line = Line::from(Span::styled(header_text, Style::default().fg(header_color)));
    lines.push(header_line);
    
    // Coin list with toggle indicators (horizontal layout)
    let mut coin_line = String::new();
    for (i, coin) in app.available_coins.iter().enumerate() {
        let indicator = if i == app.selected_coin_index { "●" } else { "○" };
        let change = (coin.change_24h / coin.price) * 100.0;
        let change_arrow = if change >= 0.0 { "↗" } else { "↘" };
        
        coin_line.push_str(&format!("{} {} ${:.0} {} {:+.1}%  ", 
            indicator, coin.symbol, coin.price, change_arrow, change));
    }
    
    let coin_line_color = Color::Magenta;
    let coin_line_span = Line::from(Span::styled(coin_line, Style::default().fg(coin_line_color)));
    lines.push(coin_line_span);
    
    // Controls line
    let controls_text = "n/N: next | v/V: prev | 1-3: select";
    let controls_line = Line::from(Span::styled(controls_text, Style::default().fg(Color::Cyan)));
    lines.push(controls_line);

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Coin Switcher"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_left_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Content
        ])
        .split(area);

    let title = Paragraph::new("Order Book")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    f.render_widget(title, chunks[0]);

    match app.selected_tab {
        0 => draw_order_book(f, app, chunks[1]),
        1 => draw_trading_panel(f, app, chunks[1]),
        2 => draw_market_data_panel(f, app, chunks[1]),
        3 => draw_orders_panel(f, app, chunks[1]),
        4 => draw_charts_panel(f, app, chunks[1]),
        5 => draw_alerts_panel(f, app, chunks[1]),
        6 => draw_settings_panel(f, app, chunks[1]),
        _ => {}
    }
}

fn draw_right_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Content
        ])
        .split(area);

    let title = Paragraph::new("Market Information")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

    f.render_widget(title, chunks[0]);

            match app.selected_tab {
            0 => draw_market_summary(f, app, chunks[1]),
            1 => draw_order_form(f, app, chunks[1]),
            2 => draw_market_details(f, app, chunks[1]),
            3 => draw_order_status(f, app, chunks[1]),
            4 => draw_price_chart(f, app, chunks[1]),
            5 => draw_websocket_status(f, app, chunks[1]),
            6 => draw_configuration(f, app, chunks[1]),
            _ => {}
        }
}

fn draw_order_book(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header with current price
            Constraint::Min(0),     // Order book content
        ])
        .split(area);

    // Draw current price header
    draw_current_price_header(f, app, chunks[0]);
    
    // Draw order book content
    draw_order_book_content(f, app, chunks[1]);
}

fn draw_current_price_header(f: &mut Frame, app: &App, area: Rect) {
    let current_price = app.market_data.current_price;
    let price_change = app.market_data.price_change;
    let price_change_percent = app.market_data.price_change_percent;
    
    let change_symbol = if price_change >= 0.0 { "↗" } else { "↘" };
    let change_color = if price_change >= 0.0 { Color::Green } else { Color::Red };
    
    let price_text = format!("${:.2}", current_price);
    let change_text = format!("{} ${:.2} ({:+.2}%)", change_symbol, price_change.abs(), price_change_percent);
    
    let header_content = vec![
        Line::from(vec![
            Span::styled("Current Price: ", Style::default().fg(Color::White)),
            Span::styled(price_text, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Change: ", Style::default().fg(Color::White)),
            Span::styled(change_text, Style::default().fg(change_color)),
        ]),
    ];

    let header = Paragraph::new(header_content)
        .block(Block::default().borders(Borders::ALL).title("Market Price"))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(header, area);
}

fn draw_order_book_content(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),  // Column headers (increased for 2 lines)
            Constraint::Min(0),     // Order data
        ])
        .split(area);

    // Draw column headers
    draw_order_book_headers(f, chunks[0]);
    
    // Draw order data
    draw_order_book_data(f, app, chunks[1]);
}

fn draw_order_book_headers(f: &mut Frame, area: Rect) {
    let header_content = vec![
        Line::from(vec![
            Span::styled("Price (USDT)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled("Amount (BTC)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled("Total (USDT)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled("Depth", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("SELL ORDERS", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled("", Style::default()),
            Span::styled("  ", Style::default()),
            Span::styled("", Style::default()),
            Span::styled("  ", Style::default()),
            Span::styled("", Style::default()),
        ]),
    ];

    let header = Paragraph::new(header_content)
        .block(Block::default().borders(Borders::NONE))
        .alignment(ratatui::layout::Alignment::Left);

    f.render_widget(header, area);
}

fn draw_order_book_data(f: &mut Frame, app: &App, area: Rect) {
    let (bids, asks) = app.order_book.get_market_depth(20);
    
    // Calculate total height for asks and bids
    let total_height = area.height as usize;
    let asks_height = (total_height / 2).min(asks.len());
    let bids_height = (total_height / 2).min(bids.len());
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(asks_height as u16),  // Asks (sell orders)
            Constraint::Length(3),                   // Current price separator
            Constraint::Length(1),                   // Buy orders label
            Constraint::Length(bids_height as u16),  // Bids (buy orders)
        ])
        .split(area);

    // Draw asks (sell orders) - red, descending order
    draw_asks_section(f, &asks, chunks[0]);
    
    // Draw current price separator with more detail
    draw_current_price_separator(f, app, chunks[1]);
    
    // Draw buy orders label
    draw_buy_orders_label(f, chunks[2]);
    
    // Draw bids (buy orders) - green, descending order
    draw_bids_section(f, &bids, chunks[3]);
}

fn draw_buy_orders_label(f: &mut Frame, area: Rect) {
    let label_content = vec![
        Line::from(vec![
            Span::styled("BUY ORDERS", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled("", Style::default()),
            Span::styled("  ", Style::default()),
            Span::styled("", Style::default()),
        ]),
    ];

    let label = Paragraph::new(label_content)
        .block(Block::default().borders(Borders::NONE))
        .alignment(ratatui::layout::Alignment::Left);

    f.render_widget(label, area);
}

fn draw_current_price_separator(f: &mut Frame, app: &App, area: Rect) {
    let current_price = app.market_data.current_price;
    let price_change = app.market_data.price_change;
    let price_change_percent = app.market_data.price_change_percent;
    
    let change_symbol = if price_change >= 0.0 { "↗" } else { "↘" };
    let change_color = if price_change >= 0.0 { Color::Green } else { Color::Red };
    
    let price_text = format!("{:.2}", current_price);
    let change_text = format!("{} ${:.2} ({:+.2}%)", change_symbol, price_change.abs(), price_change_percent);
    
    let separator_content = vec![
        Line::from(vec![
            Span::styled("─".repeat(area.width as usize), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Current: ", Style::default().fg(Color::White)),
            Span::styled(price_text, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" USDT", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled(change_text, Style::default().fg(change_color)),
        ]),
    ];

    let separator = Paragraph::new(separator_content)
        .block(Block::default().borders(Borders::NONE))
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(separator, area);
}

fn draw_asks_section(f: &mut Frame, asks: &[(f64, f64)], area: Rect) {
    let mut rows = Vec::new();
    
    // Calculate cumulative totals for background intensity
    let mut cumulative_total = 0.0;
    let max_total = asks.iter().map(|(_, qty)| qty).sum::<f64>();
    
    // Add asks in descending order (highest price first)
    for (price, quantity) in asks.iter().rev() {
        let total = price * quantity;
        cumulative_total += quantity;
        let intensity = (cumulative_total / max_total).min(1.0);
        
        // Create depth bar visualization
        let bar_length = (intensity * 20.0) as usize;
        let depth_bar = "█".repeat(bar_length);
        
        let row = Row::new(vec![
            format!("{:.2}", price),
            format!("{:.5}", quantity),
            format!("{:.2}", total),
            format!("{}", depth_bar),
        ]);
        rows.push(row);
    }

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::Red))
        .highlight_style(Style::default().fg(Color::White).bg(Color::Red));

    f.render_widget(table, area);
}

fn draw_bids_section(f: &mut Frame, bids: &[(f64, f64)], area: Rect) {
    let mut rows = Vec::new();
    
    // Calculate cumulative totals for background intensity
    let mut cumulative_total = 0.0;
    let max_total = bids.iter().map(|(_, qty)| qty).sum::<f64>();
    
    // Add bids in descending order (highest price first)
    for (price, quantity) in bids {
        let total = price * quantity;
        cumulative_total += quantity;
        let intensity = (cumulative_total / max_total).min(1.0);
        
        // Create depth bar visualization
        let bar_length = (intensity * 20.0) as usize;
        let depth_bar = "█".repeat(bar_length);
        
        let row = Row::new(vec![
            format!("{:.2}", price),
            format!("{:.5}", quantity),
            format!("{:.2}", total),
            format!("{}", depth_bar),
        ]);
        rows.push(row);
    }

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::Green))
        .highlight_style(Style::default().fg(Color::White).bg(Color::Green));

    f.render_widget(table, area);
}

fn draw_trading_panel(f: &mut Frame, app: &App, area: Rect) {
    let content = format!(
        "Trading Panel - {}\n\n\
        Best Bid: ${:.2}\n\
        Best Ask: ${:.2}\n\
        Spread: ${:.2}\n\
        Last Price: ${:.2}\n\
        Volume 24h: ${:.0}",
        app.current_market,
        app.order_book.get_best_bid().unwrap_or(0.0),
        app.order_book.get_best_ask().unwrap_or(0.0),
        app.order_book.get_spread().unwrap_or(0.0),
        app.market_data.current_price,
        app.market_data.volume_24h
    );

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Trading Overview"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_market_data_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut content = String::new();
    
    content.push_str(&format!("Market Data - {}\n\n", app.current_market));
    content.push_str(&format!("Current Price: ${:.2}\n", app.market_data.current_price));
    
    // Price change with color indication
    let price_change_text = format_number_with_color(app.market_data.price_change, false);
    let _price_change_color = get_number_color(app.market_data.price_change);
    
    let price_change_percent_text = format_number_with_color(app.market_data.price_change_percent, true);
    let _price_change_percent_color = get_number_color(app.market_data.price_change_percent);
    
    content.push_str(&format!("Change: ${} ({})\n", price_change_text, price_change_percent_text));
    content.push_str(&format!("High 24h: ${:.2}\n", app.market_data.high_24h));
    content.push_str(&format!("Low 24h: ${:.2}\n", app.market_data.low_24h));
    content.push_str(&format!("Volume 24h: ${:.0}\n", app.market_data.volume_24h));
    content.push_str(&format!("Market Cap: ${:.0}B\n", app.market_data.market_cap / 1e9));
    content.push_str(&format!("Last Update: {}", app.last_update.format("%H:%M:%S")));

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Market Data"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_orders_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut rows = Vec::new();
    rows.push(Row::new(vec!["Time", "Side", "Price", "Qty", "Status", "ID"]));

    for order in app.order_history.iter().rev().take(10) {
        rows.push(Row::new(vec![
            order.timestamp.format("%H:%M:%S").to_string(),
            format!("{:?}", order.side),
            format!("${:.2}", order.price),
            format!("{:.2}", order.quantity),
            order.status.clone(),
            order.order_id.clone(),
        ]));
    }

    let widths = [
        Constraint::Percentage(16),
        Constraint::Percentage(16),
        Constraint::Percentage(16),
        Constraint::Percentage(16),
        Constraint::Percentage(16),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths)
        .block(Block::default().borders(Borders::ALL).title("Order History"))
        .style(Style::default().fg(Color::White));

    f.render_widget(table, area);
}

fn draw_charts_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Chart content
        ])
        .split(area);

    // Header
    let header = Paragraph::new("📊 Advanced Charts - BTC/USDT")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(header, chunks[0]);

    // Chart content
    draw_price_chart(f, app, chunks[1]);
}

fn draw_settings_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();
    
    // Coin Switcher Section
    let header_text = "🪙 Coin Switcher";
    let header_line = Line::from(Span::styled(header_text, Style::default().fg(Color::Yellow)));
    lines.push(header_line);
    
    let current_text = format!("Current: {} ({})", 
        app.available_coins[app.selected_coin_index].symbol,
        app.available_coins[app.selected_coin_index].name);
    let current_line = Line::from(Span::styled(current_text, Style::default().fg(Color::Cyan)));
    lines.push(current_line);
    
    let available_text = "Available Coins:";
    let available_line = Line::from(Span::styled(available_text, Style::default().fg(Color::White)));
    lines.push(available_line);
    
    for (i, coin) in app.available_coins.iter().enumerate() {
        let indicator = if i == app.selected_coin_index { "●" } else { "○" };
        let status = if i == app.selected_coin_index { "SELECTED" } else { "       " };
        let change_percent = (coin.change_24h / coin.price) * 100.0;
        let change_color = get_number_color(change_percent);
        
        let coin_text = format!("{} {} {} - ${:.2} ({:+.2}%)", 
            indicator, coin.symbol, status, coin.price, change_percent);
        let coin_line = Line::from(Span::styled(coin_text, Style::default().fg(change_color)));
        lines.push(coin_line);
    }
    
    // Controls section
    let controls_header = "\nCoin Controls:";
    let controls_header_line = Line::from(Span::styled(controls_header, Style::default().fg(Color::Yellow)));
    lines.push(controls_header_line);
    
    let controls_text = "• n/N: Next coin\n• v/V: Previous coin\n• 1-3: Quick coin select";
    let controls_line = Line::from(Span::styled(controls_text, Style::default().fg(Color::Cyan)));
    lines.push(controls_line);
    
    // Settings section
    let settings_header = "\nSettings:";
    let settings_header_line = Line::from(Span::styled(settings_header, Style::default().fg(Color::Yellow)));
    lines.push(settings_header_line);
    
    let settings_text = format!("Current Market: {}\nPolymarket Client: {}\nOrder Input Mode: {}\nHelp Mode: {}\nAuto-refresh: Enabled\nNotifications: Enabled\nTheme: Dark\nLanguage: English",
        app.current_market,
        if app.polymarket_client.is_some() { "Connected" } else { "Disconnected" },
        if app.order_input.active { "Active" } else { "Inactive" },
        if app.help_mode { "On" } else { "Off" }
    );
    let settings_line = Line::from(Span::styled(settings_text, Style::default().fg(Color::White)));
    lines.push(settings_line);

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Settings & Coin Switcher"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_market_summary(f: &mut Frame, app: &App, area: Rect) {
    let mut content = String::new();
    
    content.push_str("Market Summary\n\n");
    content.push_str(&format!("Symbol: {}\n", app.current_market));
    content.push_str(&format!("Current Price: ${:.2}\n", app.market_data.current_price));
    
    // Price change with color indication
    let price_change_text = format_number_with_color(app.market_data.price_change, false);
    let price_change_percent_text = format_number_with_color(app.market_data.price_change_percent, true);
    
    content.push_str(&format!("Change: ${} ({})\n", price_change_text, price_change_percent_text));
    content.push_str(&format!("Volume: ${:.0}\n", app.market_data.volume_24h));
    content.push_str(&format!("Market Cap: ${:.0}B", app.market_data.market_cap / 1e9));

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Summary"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_order_form(f: &mut Frame, app: &App, area: Rect) {
    let content = format!(
        "Order Form\n\n\
        Side: {:?}\n\
        Price: ${}\n\
        Quantity: {}\n\
        Type: {:?}\n\
        Token: {}\n\
        Status: {}\n\n\
        Controls:\n\
        b/s - Change side\n\
        g/f/d - Change type\n\
        Enter - Submit order",
        app.order_input.side,
        app.order_input.price,
        app.order_input.quantity,
        app.order_input.order_type,
        app.order_input.token_id,
        if app.order_input.active { "ACTIVE" } else { "Inactive" }
    );

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Place Order"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_market_details(f: &mut Frame, app: &App, area: Rect) {
    let content = format!(
        "Market Details\n\n\
        High 24h: ${:.2}\n\
        Low 24h: ${:.2}\n\
        Open: ${:.2}\n\
        Previous Close: ${:.2}\n\
        Bid Size: {:.2}\n\
        Ask Size: {:.2}\n\
        Spread: ${:.4}\n\
        Spread %: {:.2}%",
        app.market_data.high_24h,
        app.market_data.low_24h,
        app.market_data.current_price - app.market_data.price_change,
        app.market_data.current_price - app.market_data.price_change,
        app.order_book.get_best_bid().map_or(0.0, |_| 10.0),
        app.order_book.get_best_ask().map_or(0.0, |_| 12.0),
        app.order_book.get_spread().unwrap_or(0.0),
        app.order_book.get_spread().map_or(0.0, |s| (s / app.market_data.current_price) * 100.0)
    );

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_order_status(f: &mut Frame, app: &App, area: Rect) {
    let content = format!(
        "Order Status\n\n\
        Total Orders: {}\n\
        Pending: {}\n\
        Filled: {}\n\
        Cancelled: {}\n\
        Last Order: {}\n\
        Success Rate: {:.1}%",
        app.order_history.len(),
        app.order_history.iter().filter(|o| o.status == "Pending").count(),
        app.order_history.iter().filter(|o| o.status == "Filled").count(),
        app.order_history.iter().filter(|o| o.status == "Cancelled").count(),
        app.order_history.back().map_or("None".to_string(), |o| o.timestamp.format("%H:%M:%S").to_string()),
        if app.order_history.is_empty() { 0.0 } else { 
            (app.order_history.iter().filter(|o| o.status == "Filled").count() as f64 / app.order_history.len() as f64) * 100.0 
        }
    );

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_price_chart(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Chart
        ])
        .split(area);

    // Header with timeframe and current price
    let header = format!(
        "📊 BTC/USDT Price Chart - {} | Current: ${:.2} | Change: {:.2} ({:.2}%)",
        app.selected_timeframe.as_str(),
        app.market_data.current_price,
        app.market_data.price_change,
        app.market_data.price_change_percent
    );
    
    let header_para = Paragraph::new(header)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(header_para, chunks[0]);

    // Chart content using terminal chart backend
    if app.candlestick_data.len() < 2 {
        let content = "Insufficient data for chart";
        let paragraph = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("Chart"))
            .wrap(Wrap { trim: true });
        
        f.render_widget(paragraph, chunks[1]);
        return;
    }

    // Resize terminal chart to fit the area
    let chart_width = chunks[1].width.saturating_sub(2) as u32;
    let chart_height = chunks[1].height.saturating_sub(2) as u32;
    
    app.resize_terminal_chart(chart_width, chart_height);

    // Update terminal chart with current data
    let _ = app.terminal_chart.draw_candlestick_chart(
        &app.candlestick_data, 
        app.market_data.current_price
    );

    // Render the terminal chart
    let chart_content = app.terminal_chart.render();
    let paragraph = Paragraph::new(chart_content)
        .block(Block::default().borders(Borders::ALL).title("Real-time Chart"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, chunks[1]);
}

fn draw_configuration(f: &mut Frame, app: &App, area: Rect) {
    let content = format!(
        "Configuration\n\n\
        API Endpoint: {}\n\
        Chain ID: 137 (Polygon)\n\
        Signature Type: {:?}\n\
        Auto-refresh: Enabled\n\
        Notifications: Enabled\n\
        Logging: Debug\n\
        Theme: Dark\n\
        Language: English\n\
        Timezone: UTC",
        if app.polymarket_client.is_some() { "https://clob.polymarket.com" } else { "Not configured" },
        PolymarketSignatureType::EMAIL_MAGIC
    );

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Config"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_bottom_bar(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),  // User commands
            Constraint::Percentage(25),  // Navigation help
            Constraint::Percentage(20),  // Current coin info
            Constraint::Percentage(25),  // Real-time updates
        ])
        .split(area);

    // User commands area
    let command_text = if app.user_command.is_empty() {
        "Type commands here... (h for help)".to_string()
    } else {
        format!("Command: {}", app.user_command)
    };

    let command_para = Paragraph::new(command_text)
        .block(Block::default().borders(Borders::ALL).title("User Commands (Type here and press Enter)"))
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(command_para, chunks[0]);

    // Navigation help area
    let help_text = format!(
        "Tab: {} | F2-F8: Quick tabs | ?/H: Help | P/Space: Order input | M: Market data | R: Refresh | N/V: Coin switch | </>: Timeframe | L: Auto-refresh | W: Real data",
        app.tabs[app.selected_tab]
    );

    let help_para = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Navigation Help"))
        .wrap(Wrap { trim: true });

    f.render_widget(help_para, chunks[1]);

    // Current coin info area
    let selected_coin = &app.available_coins[app.selected_coin_index];
    let change_percent = (selected_coin.change_24h / selected_coin.price) * 100.0;
    let change_color = get_number_color(change_percent);
    
    let coin_text = format!(
        "🪙 {} ({})\n${:.2} {:+.2}%\nVolume: ${:.0}M",
        selected_coin.symbol,
        selected_coin.name,
        selected_coin.price,
        change_percent,
        selected_coin.volume_24h / 1e6
    );

    let coin_para = Paragraph::new(coin_text)
        .block(Block::default().borders(Borders::ALL).title("Active Coin"))
        .style(Style::default().fg(change_color))
        .wrap(Wrap { trim: true });

    f.render_widget(coin_para, chunks[2]);

    // Real-time updates area with status
    let status_color = if app.real_time_service.is_connected { Color::Green } else { Color::Red };
    let status_text = format!(
        "Status: {}\nTimeframe: {}\nAuto-refresh: {}\nUpdates: {}\nAlerts: {}",
        app.real_time_service.connection_status,
        app.selected_timeframe.as_str(),
        if app.auto_refresh { "ON" } else { "OFF" },
        app.real_time_data.len(),
        app.get_active_alerts_count()
    );

    let updates_text = if app.real_time_data.is_empty() {
        "No updates yet...".to_string()
    } else {
        app.real_time_data.iter().take(3).cloned().collect::<Vec<_>>().join("\n")
    };

    let full_text = format!("{}\n\n{}", status_text, updates_text);

    let updates_para = Paragraph::new(full_text)
        .block(Block::default().borders(Borders::ALL).title("Real-time Status & Updates"))
        .style(Style::default().fg(status_color))
        .wrap(Wrap { trim: true });

    f.render_widget(updates_para, chunks[3]);
}

fn draw_alerts_panel(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Alerts list
        ])
        .split(area);

    // Header
    let header = Paragraph::new(format!(
        "🔔 Price Alerts - Active: {} | Triggered: {}",
        app.get_active_alerts_count(),
        app.get_triggered_alerts_count()
    ))
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(header, chunks[0]);

    // Alerts list
    if app.price_alerts.is_empty() {
        let content = "No price alerts configured.\n\nUse the command line to create alerts:\n• alert above 27000 - Alert when price goes above $27,000\n• alert below 26000 - Alert when price goes below $26,000\n• alert change 5 - Alert on 5% price change\n• alert volume 1000000 - Alert on volume spike above 1M";
        
        let paragraph = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("Create Alerts"))
            .wrap(Wrap { trim: true });
        
        f.render_widget(paragraph, chunks[1]);
    } else {
        let mut rows = Vec::new();
        rows.push(Row::new(vec!["ID", "Symbol", "Type", "Target", "Status", "Created", "Triggered"]));

        for alert in &app.price_alerts {
            let alert_type_str = match &alert.alert_type {
                AlertType::PriceAbove(price) => format!("Above ${:.2}", price),
                AlertType::PriceBelow(price) => format!("Below ${:.2}", price),
                AlertType::PercentageChange(percent) => format!("{}% Change", percent),
                AlertType::VolumeSpike(volume) => format!("Volume > {:.0}", volume),
                AlertType::PriceCross(price) => format!("Cross ${:.2}", price),
            };
            
            let status = if alert.is_active { "🟢 Active" } else { "🔴 Inactive" };
            let created = alert.created_at.format("%H:%M").to_string();
            let triggered = alert.triggered_at
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_else(|| "Never".to_string());
            
            rows.push(Row::new(vec![
                alert.id.to_string(),
                alert.symbol.clone(),
                alert_type_str,
                alert.message.clone(),
                status.to_string(),
                created,
                triggered,
            ]));
        }

        let widths = [
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Length(15),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(10),
        ];

        let table = Table::new(rows, widths)
            .block(Block::default().borders(Borders::ALL).title("Price Alerts"))
            .style(Style::default().fg(Color::White));

        f.render_widget(table, chunks[1]);
    }
}

fn draw_websocket_status(f: &mut Frame, app: &App, area: Rect) {
    let mut content = String::new();
    
    content.push_str("🌐 Binance WebSocket Status\n\n");
    
    // Connection status
    let status_icon = if app.binance_ws.is_connected { "🟢" } else { "🔴" };
    content.push_str(&format!("Status: {} {}\n", status_icon, app.binance_ws.connection_status));
    
    // Data mode
    let mode_icon = if app.use_real_data { "📡" } else { "🎭" };
    content.push_str(&format!("Data Mode: {} {}\n", mode_icon, 
        if app.use_real_data { "Real Binance Data" } else { "Simulated Data" }));
    
    // Statistics
    content.push_str(&format!("Messages Received: {}\n", app.binance_ws.message_count));
    content.push_str(&format!("Errors: {}\n", app.binance_ws.error_count));
    content.push_str(&format!("Last Message: {}\n", 
        app.binance_ws.last_message.format("%H:%M:%S")));
    
    // Connection info
    content.push_str("\n📊 Connection Info:\n");
    content.push_str(&format!("• Market: {}\n", app.current_market));
    content.push_str(&format!("• Timeframe: {}\n", app.selected_timeframe.as_str()));
    content.push_str(&format!("• Auto-refresh: {}\n", 
        if app.auto_refresh { "ON" } else { "OFF" }));
    
    // Controls
    content.push_str("\n🎮 Controls:\n");
    content.push_str("• r/R: Toggle real/simulated data\n");
    content.push_str("• b/B: Simulate Binance connection\n");
    content.push_str("• < >: Change chart timeframe\n");
    content.push_str("• l/L: Toggle auto-refresh\n");
    
    // Alert info
    content.push_str(&format!("\n🔔 Alerts: {} active, {} triggered\n", 
        app.get_active_alerts_count(), app.get_triggered_alerts_count()));
    
    // Real-time updates
    if !app.real_time_data.is_empty() {
        content.push_str("\n📡 Recent Updates:\n");
        for update in app.real_time_data.iter().take(5) {
            content.push_str(&format!("• {}\n", update));
        }
    }

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("WebSocket & Real-time Status"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}
