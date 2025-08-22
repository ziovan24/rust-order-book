use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use crate::order_book::OrderBook;
use crate::order::OrderSide;

pub struct BinanceWebSocketClient {
    pub symbol: String,
    pub order_book: Arc<OrderBook>,
    pub base_url: String,
    pub ping_interval: Duration,
    pub last_ping: Instant,
    pub last_pong: Instant,
    pub connection_id: u64,
    pub is_connected: bool,
    pub reconnect_attempts: u32,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay: Duration,
    pub depth_snapshot: Option<DepthSnapshot>,
    pub buffered_events: Vec<DepthUpdateEvent>,
    pub last_update_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DepthSnapshot {
    pub lastUpdateId: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DepthUpdateEvent {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradeEvent {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookTickerEvent {
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub best_bid_price: String,
    #[serde(rename = "B")]
    pub best_bid_qty: String,
    #[serde(rename = "a")]
    pub best_ask_price: String,
    #[serde(rename = "A")]
    pub best_ask_qty: String,
}

#[derive(Debug, Serialize)]
pub struct BinanceSubscribeRequest {
    pub method: String,
    pub params: Vec<String>,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct BinanceSubscribeResponse {
    pub result: Option<Vec<String>>,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct BinanceErrorResponse {
    pub code: i32,
    pub msg: String,
    pub id: Option<u64>,
}

impl BinanceWebSocketClient {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            order_book: Arc::new(OrderBook::new()),
            base_url: "wss://stream.binance.com:9443".to_string(),
            ping_interval: Duration::from_secs(20),
            last_ping: Instant::now(),
            last_pong: Instant::now(),
            connection_id: 0,
            is_connected: false,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(1),
            depth_snapshot: None,
            buffered_events: Vec::new(),
            last_update_id: 0,
        }
    }

    pub fn simulate_binance_connection(&self) {
        println!("🔌 Simulating Binance WebSocket connection...");
        println!("📡 Would connect to: {}/ws/{}@depth@100ms", 
            self.base_url, self.symbol.to_lowercase());
        println!("📊 Would subscribe to: depth updates, trades, book ticker");
        println!("🏓 Would handle ping/pong every 20 seconds");
        println!("🔄 Would reconnect automatically on disconnection");
    }

    pub fn display_order_book(&self) {
        println!("\n📊 Real-time Order Book for {}:", self.symbol);
        println!("{}", self.order_book);
        
        if let Some(spread) = self.order_book.get_spread() {
            println!("📈 Current Spread: {:.8}", spread);
        }
        
        if let (Some(best_bid), Some(best_ask)) = (self.order_book.get_best_bid(), self.order_book.get_best_ask()) {
            println!("💰 Best Bid: {:.8} | Best Ask: {:.8}", best_bid, best_ask);
        }
        
        let (bids, asks) = self.order_book.get_market_depth(5);
        println!("📊 Top 5 Bids: {:?}", bids);
        println!("📊 Top 5 Asks: {:?}", asks);
        
        println!("🔌 Connection: {} (ID: {})", 
            if self.is_connected { "✅ Connected" } else { "❌ Disconnected" }, 
            self.connection_id);
        
        println!("{}", "─".repeat(60));
    }
}

pub async fn run_binance_client(symbol: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = BinanceWebSocketClient::new(symbol.clone());
    
    println!("🚀 Starting Binance WebSocket client for {}", symbol);
    println!("🔌 Base URL: {}", client.base_url);
    println!("📡 Streams: depth@100ms, trade, bookTicker");
    println!("🏓 Ping/Pong: Every 20 seconds");
    println!("🔄 Auto-reconnect: Enabled");
    println!("⚠️  Note: This is a simulated client for demonstration");
    
    client.simulate_binance_connection();
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_client_creation() {
        let client = BinanceWebSocketClient::new("BTCUSDT".to_string());
        assert_eq!(client.symbol, "BTCUSDT");
        assert_eq!(client.order_book.get_total_orders(), 0);
        assert_eq!(client.order_book.get_total_price_levels(), (0, 0));
    }

    #[test]
    fn test_subscribe_request_serialization() {
        let request = BinanceSubscribeRequest {
            method: "SUBSCRIBE".to_string(),
            params: vec!["btcusdt@depth20@100ms".to_string()],
            id: 1,
        };
        
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("SUBSCRIBE"));
        assert!(json.contains("btcusdt@depth20@100ms"));
    }

    #[test]
    fn test_depth_snapshot_deserialization() {
        let json = r#"{
            "lastUpdateId": 12345,
            "bids": [["50000.00", "1.5"], ["49999.00", "2.0"]],
            "asks": [["50001.00", "1.0"], ["50002.00", "2.5"]]
        }"#;
        
        let snapshot: DepthSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.lastUpdateId, 12345);
        assert_eq!(snapshot.bids.len(), 2);
        assert_eq!(snapshot.asks.len(), 2);
    }
}
