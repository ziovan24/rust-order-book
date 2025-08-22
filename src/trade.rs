#[derive(Debug, Clone)]
pub struct Trade {
    pub bid_order_id: u64,
    pub ask_order_id: u64,
    pub price: f64,
    pub quantity: f64,
    pub timestamp: u64,
}
