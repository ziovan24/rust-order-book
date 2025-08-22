use crate::price::Price;

#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: u64,
    pub side: OrderSide,
    pub price: Price,
    pub quantity: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Bid,
    Ask,
}

impl Order {
    pub fn new(id: u64, side: OrderSide, price: f64, quantity: f64, timestamp: u64) -> Self {
        Self {
            id,
            side,
            price: Price(price),
            quantity,
            timestamp,
        }
    }
}

