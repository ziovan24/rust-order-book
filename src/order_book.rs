use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use parking_lot::RwLock;
use crate::order::{Order, OrderSide};
use crate::price::Price;
use crate::trade::Trade;

#[derive(Debug)]
pub struct OrderQueue {
    orders: DashMap<u64, Order>,
    order_queue: SegQueue<u64>,
    total_quantity: AtomicUsize,
}

impl OrderQueue {
    pub fn new() -> Self {
        Self {
            orders: DashMap::new(),
            order_queue: SegQueue::new(),
            total_quantity: AtomicUsize::new(0),
        }
    }

    pub fn add_order(&self, order: Order) {
        let quantity = (order.quantity * 1_000_000.0) as usize;
        self.orders.insert(order.id, order.clone());
        self.order_queue.push(order.id);
        self.total_quantity.fetch_add(quantity, Ordering::Relaxed);
    }

    pub fn remove_order(&self, order_id: u64) -> Option<Order> {
        if let Some((_, order)) = self.orders.remove(&order_id) {
            let quantity = (order.quantity * 1_000_000.0) as usize;
            self.total_quantity.fetch_sub(quantity, Ordering::Relaxed);
            Some(order)
        } else {
            None
        }
    }

    pub fn update_order(&self, order_id: u64, new_quantity: f64) -> bool {
        if let Some(mut order_ref) = self.orders.get_mut(&order_id) {
            let old_quantity = (order_ref.quantity * 1_000_000.0) as usize;
            let new_quantity_int = (new_quantity * 1_000_000.0) as usize;
            
            order_ref.quantity = new_quantity;
            self.total_quantity.fetch_add(new_quantity_int, Ordering::Relaxed);
            self.total_quantity.fetch_sub(old_quantity, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn get_total_quantity(&self) -> f64 {
        (self.total_quantity.load(Ordering::Relaxed) as f64) / 1_000_000.0
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn len(&self) -> usize {
        self.orders.len()
    }

    pub fn get_first_order(&self) -> Option<Order> {
        let mut temp_queue = Vec::new();
        let mut first_order = None;
        
        while let Some(order_id) = self.order_queue.pop() {
            if let Some(order) = self.orders.get(&order_id) {
                first_order = Some(order.clone());
                temp_queue.push(order_id);
                break;
            }
            temp_queue.push(order_id);
        }
        
        for order_id in temp_queue {
            self.order_queue.push(order_id);
        }
        
        first_order
    }

    pub fn remove_first_order(&self) -> Option<Order> {
        while let Some(order_id) = self.order_queue.pop() {
            if let Some(order) = self.remove_order(order_id) {
                return Some(order);
            }
        }
        None
    }

    pub fn get_all_orders(&self) -> Vec<Order> {
        self.orders.iter().map(|entry| entry.value().clone()).collect()
    }

    pub fn match_orders_with(&self, other_queue: &OrderQueue, side: OrderSide) -> Vec<Trade> {
        let mut trades = Vec::new();
        
        let self_orders = self.get_all_orders();
        let other_orders = other_queue.get_all_orders();
        
        if self_orders.is_empty() || other_orders.is_empty() {
            return trades;
        }
        
        let mut self_sorted = self_orders.clone();
        let mut other_sorted = other_orders.clone();
        
        match side {
            OrderSide::Bid => {
                self_sorted.sort_by(|a, b| {
                    b.price.partial_cmp(&a.price).unwrap()
                        .then(a.timestamp.cmp(&b.timestamp))
                });
                other_sorted.sort_by(|a, b| {
                    a.price.partial_cmp(&b.price).unwrap()
                        .then(a.timestamp.cmp(&b.timestamp))
                });
            }
            OrderSide::Ask => {
                self_sorted.sort_by(|a, b| {
                    a.price.partial_cmp(&b.price).unwrap()
                        .then(a.timestamp.cmp(&b.timestamp))
                });
                other_sorted.sort_by(|a, b| {
                    b.price.partial_cmp(&a.price).unwrap()
                        .then(a.timestamp.cmp(&b.timestamp))
                });
            }
        }
        
        let mut self_idx = 0;
        let mut other_idx = 0;
        
        while self_idx < self_sorted.len() && other_idx < other_sorted.len() {
            let self_order = &self_sorted[self_idx];
            let other_order = &other_sorted[other_idx];
            
            let can_match = match side {
                OrderSide::Bid => self_order.price.as_f64() >= other_order.price.as_f64(),
                OrderSide::Ask => self_order.price.as_f64() <= other_order.price.as_f64(),
            };
            
            if can_match {
                let trade_quantity = self_order.quantity.min(other_order.quantity);
                let trade_price = if self_order.timestamp <= other_order.timestamp {
                    self_order.price.as_f64()
                } else {
                    other_order.price.as_f64()
                };
                
                trades.push(Trade {
                    bid_order_id: if side == OrderSide::Bid { self_order.id } else { other_order.id },
                    ask_order_id: if side == OrderSide::Bid { other_order.id } else { self_order.id },
                    price: trade_price,
                    quantity: trade_quantity,
                    timestamp: std::cmp::min(self_order.timestamp, other_order.timestamp),
                });
                
                if self_order.quantity <= other_order.quantity {
                    self_idx += 1;
                } else {
                    other_idx += 1;
                }
            } else {
                break;
            }
        }
        
        trades
    }
}

#[derive(Debug, Clone)]
pub struct PriceLevel {
    pub price: Price,
    pub orders: Arc<OrderQueue>,
}

impl PriceLevel {
    pub fn new(price: f64) -> Self {
        Self {
            price: Price(price),
            orders: Arc::new(OrderQueue::new()),
        }
    }

    pub fn add_order(&self, order: Order) {
        self.orders.add_order(order);
    }

    pub fn remove_order(&self, order_id: u64) -> Option<Order> {
        self.orders.remove_order(order_id)
    }

    pub fn update_order(&self, order_id: u64, new_quantity: f64) -> bool {
        self.orders.update_order(order_id, new_quantity)
    }

    pub fn get_total_quantity(&self) -> f64 {
        self.orders.get_total_quantity()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn len(&self) -> usize {
        self.orders.len()
    }

    pub fn get_first_order(&self) -> Option<Order> {
        self.orders.get_first_order()
    }

    pub fn remove_first_order(&self) -> Option<Order> {
        self.orders.remove_first_order()
    }
}

#[derive(Debug)]
pub struct OrderBook {
    bids: RwLock<BTreeMap<Price, PriceLevel>>,
    asks: RwLock<BTreeMap<Price, PriceLevel>>,
    next_order_id: AtomicU64,
    stats: Arc<RwLock<OrderBookStats>>,
    matching_lock: parking_lot::Mutex<()>,
}

#[derive(Debug, Clone)]
pub struct OrderBookStats {
    pub total_orders_created: u64,
    pub total_orders_matched: u64,
    pub total_orders_cancelled: u64,
    pub total_volume_traded: f64,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub spread: Option<f64>,
    pub mid_price: Option<f64>,
    pub last_match_time: Option<u64>,
}

impl OrderBookStats {
    pub fn new() -> Self {
        Self {
            total_orders_created: 0,
            total_orders_matched: 0,
            total_orders_cancelled: 0,
            total_volume_traded: 0.0,
            best_bid: None,
            best_ask: None,
            spread: None,
            mid_price: None,
            last_match_time: None,
        }
    }

    pub fn update_market_data(&mut self, best_bid: Option<f64>, best_ask: Option<f64>) {
        self.best_bid = best_bid;
        self.best_ask = best_ask;
        
        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
            self.spread = Some(ask - bid);
            self.mid_price = Some((bid + ask) / 2.0);
        } else {
            self.spread = None;
            self.mid_price = None;
        }
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: RwLock::new(BTreeMap::new()),
            asks: RwLock::new(BTreeMap::new()),
            next_order_id: AtomicU64::new(1),
            stats: Arc::new(RwLock::new(OrderBookStats::new())),
            matching_lock: parking_lot::Mutex::new(()),
        }
    }

    pub fn add_order(&self, side: OrderSide, price: f64, quantity: f64, timestamp: u64) -> u64 {
        let order_id = self.next_order_id.fetch_add(1, Ordering::Relaxed);
        let order = Order::new(order_id, side.clone(), price, quantity, timestamp);

        match side {
            OrderSide::Bid => {
                let mut bids = self.bids.write();
                bids.entry(Price(price))
                    .or_insert_with(|| PriceLevel::new(price))
                    .add_order(order);
            }
            OrderSide::Ask => {
                let mut asks = self.asks.write();
                asks.entry(Price(price))
                    .or_insert_with(|| PriceLevel::new(price))
                    .add_order(order);
            }
        }

        {
            let mut stats = self.stats.write();
            stats.total_orders_created += 1;
            self.update_stats_internal(&mut stats);
        }

        order_id
    }

    pub fn add_market_order(&self, side: OrderSide, quantity: f64, timestamp: u64) -> Vec<Trade> {
        let _lock = self.matching_lock.lock();
        
        let order_id = self.next_order_id.fetch_add(1, Ordering::Relaxed);
        let order = Order::new(order_id, side.clone(), 0.0, quantity, timestamp);
        
        let trades = match side {
            OrderSide::Bid => {
                self.match_market_order(order, true)
            }
            OrderSide::Ask => {
                self.match_market_order(order, false)
            }
        };
        
        if !trades.is_empty() {
            let mut stats = self.stats.write();
            stats.total_orders_created += 1;
            stats.total_orders_matched += trades.len() as u64;
            stats.total_volume_traded += trades.iter().map(|t| t.price * t.quantity).sum::<f64>();
            stats.last_match_time = Some(timestamp);
            self.update_stats_internal(&mut stats);
        }
        
        trades
    }

    fn match_market_order(&self, order: Order, is_buy: bool) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut remaining_quantity = order.quantity;
        
        if is_buy {
            loop {
                let ask_price = {
                    let asks = self.asks.read();
                    asks.keys().next().cloned()
                };
                
                if let Some(ask_price) = ask_price {
                    if remaining_quantity <= 0.0 {
                        break;
                    }
                    
                    let mut asks = self.asks.write();
                    if let Some(ask_level) = asks.get_mut(&ask_price) {
                        if let Some(ask_order) = ask_level.get_first_order() {
                            let trade_quantity = remaining_quantity.min(ask_order.quantity);
                            let trade_price = ask_order.price.as_f64();
                            
                            trades.push(Trade {
                                bid_order_id: order.id,
                                ask_order_id: ask_order.id,
                                price: trade_price,
                                quantity: trade_quantity,
                                timestamp: std::cmp::min(order.timestamp, ask_order.timestamp),
                            });
                            
                            remaining_quantity -= trade_quantity;
                            
                            if ask_order.quantity <= trade_quantity {
                                ask_level.remove_first_order();
                            } else {
                                ask_level.update_order(ask_order.id, ask_order.quantity - trade_quantity);
                            }
                            
                            if ask_level.is_empty() {
                                asks.remove(&ask_price);
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        } else {
            loop {
                let bid_price = {
                    let bids = self.bids.read();
                    bids.keys().next_back().cloned()
                };
                
                if let Some(bid_price) = bid_price {
                    if remaining_quantity <= 0.0 {
                        break;
                    }
                    
                    let mut bids = self.bids.write();
                    if let Some(bid_level) = bids.get_mut(&bid_price) {
                        if let Some(bid_order) = bid_level.get_first_order() {
                            let trade_quantity = remaining_quantity.min(bid_order.quantity);
                            let trade_price = bid_order.price.as_f64();
                            
                            trades.push(Trade {
                                bid_order_id: bid_order.id,
                                ask_order_id: order.id,
                                price: trade_price,
                                quantity: trade_quantity,
                                timestamp: std::cmp::min(order.timestamp, bid_order.timestamp),
                            });
                            
                            remaining_quantity -= trade_quantity;
                            
                            if bid_order.quantity <= trade_quantity {
                                bid_level.remove_first_order();
                            } else {
                                bid_level.update_order(bid_order.id, bid_order.quantity - trade_quantity);
                            }
                            
                            if bid_level.is_empty() {
                                bids.remove(&bid_price);
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        
        trades
    }

    pub fn remove_order(&self, order_id: u64) -> Option<Order> {
        let mut removed_order = None;
        let mut bid_price_to_remove = None;
        let mut ask_price_to_remove = None;

        {
            let mut bids = self.bids.write();
            for (price, price_level) in bids.iter_mut() {
                if let Some(order) = price_level.remove_order(order_id) {
                    removed_order = Some(order);
                    if price_level.is_empty() {
                        bid_price_to_remove = Some(price.clone());
                    }
                    break;
                }
            }
            
            if let Some(price) = bid_price_to_remove {
                bids.remove(&price);
            }
        }

        if removed_order.is_none() {
            let mut asks = self.asks.write();
            for (price, price_level) in asks.iter_mut() {
                if let Some(order) = price_level.remove_order(order_id) {
                    removed_order = Some(order);
                    if price_level.is_empty() {
                        ask_price_to_remove = Some(price.clone());
                    }
                    break;
                }
            }
            
            if let Some(price) = ask_price_to_remove {
                asks.remove(&price);
            }
        }

        if removed_order.is_some() {
            let mut stats = self.stats.write();
            stats.total_orders_cancelled += 1;
            self.update_stats_internal(&mut stats);
        }

        removed_order
    }

    pub fn update_order(&self, order_id: u64, new_quantity: f64) -> bool {
        let mut updated = false;

        {
            let bids = self.bids.read();
            for price_level in bids.values() {
                if price_level.update_order(order_id, new_quantity) {
                    updated = true;
                    break;
                }
            }
        }

        if !updated {
            let asks = self.asks.read();
            for price_level in asks.values() {
                if price_level.update_order(order_id, new_quantity) {
                    updated = true;
                    break;
                }
            }
        }

        if updated {
            let mut stats = self.stats.write();
            self.update_stats_internal(&mut stats);
        }

        updated
    }

    pub fn get_best_bid(&self) -> Option<f64> {
        let bids = self.bids.read();
        bids.keys().next_back().map(|p| p.as_f64())
    }

    pub fn get_best_ask(&self) -> Option<f64> {
        let asks = self.asks.read();
        asks.keys().next().map(|p| p.as_f64())
    }

    pub fn get_spread(&self) -> Option<f64> {
        let stats = self.stats.read();
        stats.spread
    }

    pub fn get_mid_price(&self) -> Option<f64> {
        let stats = self.stats.read();
        stats.mid_price
    }

    pub fn get_market_depth(&self, levels: usize) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let bids: Vec<(f64, f64)> = {
            let bids = self.bids.read();
            bids.iter()
                .rev()
                .take(levels)
                .map(|(price, level)| (price.as_f64(), level.get_total_quantity()))
                .collect()
        };

        let asks: Vec<(f64, f64)> = {
            let asks = self.asks.read();
            asks.iter()
                .take(levels)
                .map(|(price, level)| (price.as_f64(), level.get_total_quantity()))
                .collect()
        };

        (bids, asks)
    }

    pub fn match_orders(&self) -> Vec<Trade> {
        let _lock = self.matching_lock.lock();
        
        let mut trades = Vec::new();
        let mut total_matched = 0;
        let mut iteration_count = 0;
        const MAX_ITERATIONS: usize = 1000;

        loop {
            iteration_count += 1;
            if iteration_count > MAX_ITERATIONS {
                break;
            }

            let (best_bid, best_ask) = {
                let best_bid = self.get_best_bid();
                let best_ask = self.get_best_ask();
                (best_bid, best_ask)
            };

            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                if bid < ask {
                    break;
                }

                let bid_price = Price(bid);
                let ask_price = Price(ask);

                let (bid_level, ask_level) = {
                    let mut bids = self.bids.write();
                    let mut asks = self.asks.write();
                    
                    let bid_level = bids.get_mut(&bid_price).cloned();
                    let ask_level = asks.get_mut(&ask_price).cloned();
                    
                    (bid_level, ask_level)
                };

                if let (Some(bid_level), Some(ask_level)) = (bid_level, ask_level) {
                    if let (Some(bid_order), Some(ask_order)) = (bid_level.get_first_order(), ask_level.get_first_order()) {
                        let trade_quantity = bid_order.quantity.min(ask_order.quantity);
                        let trade_price = if bid_order.timestamp <= ask_order.timestamp {
                            bid
                        } else {
                            ask
                        };

                        trades.push(Trade {
                            bid_order_id: bid_order.id,
                            ask_order_id: ask_order.id,
                            price: trade_price,
                            quantity: trade_quantity,
                            timestamp: std::cmp::min(bid_order.timestamp, ask_order.timestamp),
                        });

                        total_matched += 1;

                        if bid_order.quantity <= ask_order.quantity {
                            bid_level.remove_first_order();
                        } else {
                            bid_level.update_order(bid_order.id, bid_order.quantity - trade_quantity);
                        }

                        if ask_order.quantity <= bid_order.quantity {
                            ask_level.remove_first_order();
                        } else {
                            ask_level.update_order(ask_order.id, ask_order.quantity - trade_quantity);
                        }

                        if bid_level.is_empty() {
                            let mut bids = self.bids.write();
                            bids.remove(&bid_price);
                        }
                        if ask_level.is_empty() {
                            let mut asks = self.asks.write();
                            asks.remove(&ask_price);
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if total_matched > 0 {
            let mut stats = self.stats.write();
            stats.total_orders_matched += total_matched;
            stats.total_volume_traded += trades.iter().map(|t| t.price * t.quantity).sum::<f64>();
            stats.last_match_time = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64);
            self.update_stats_internal(&mut stats);
        }

        trades
    }

    pub fn get_stats(&self) -> OrderBookStats {
        self.stats.read().clone()
    }

    pub fn get_total_orders(&self) -> usize {
        let bids = self.bids.read();
        let asks = self.asks.read();
        
        let bid_count: usize = bids.values().map(|level| level.len()).sum();
        let ask_count: usize = asks.values().map(|level| level.len()).sum();
        
        bid_count + ask_count
    }

    pub fn get_total_price_levels(&self) -> (usize, usize) {
        let bids = self.bids.read();
        let asks = self.asks.read();
        
        (bids.len(), asks.len())
    }

    fn update_stats_internal(&self, stats: &mut OrderBookStats) {
        let best_bid = self.get_best_bid();
        let best_ask = self.get_best_ask();
        stats.update_market_data(best_bid, best_ask);
    }

    pub fn clear(&self) {
        let mut bids = self.bids.write();
        let mut asks = self.asks.write();
        bids.clear();
        asks.clear();
        
        let mut stats = self.stats.write();
        *stats = OrderBookStats::new();
    }

    pub fn get_order(&self, order_id: u64) -> Option<Order> {
        {
            let bids = self.bids.read();
            for price_level in bids.values() {
                if let Some(order) = price_level.orders.orders.get(&order_id) {
                    return Some(order.clone());
                }
            }
        }

        {
            let asks = self.asks.read();
            for price_level in asks.values() {
                if let Some(order) = price_level.orders.orders.get(&order_id) {
                    return Some(order.clone());
                }
            }
        }

        None
    }

    pub fn validate_consistency(&self) -> bool {
        let bids = self.bids.read();
        let asks = self.asks.read();
        
        let mut prev_bid_price = f64::MAX;
        for (price, _) in bids.iter() {
            let current_price = price.as_f64();
            if current_price > prev_bid_price {
                return false;
            }
            prev_bid_price = current_price;
        }
        
        let mut prev_ask_price = f64::MIN;
        for (price, _) in asks.iter() {
            let current_price = price.as_f64();
            if current_price < prev_ask_price {
                return false;
            }
            prev_ask_price = current_price;
        }
        
        if let (Some(best_bid), Some(best_ask)) = (self.get_best_bid(), self.get_best_ask()) {
            if best_bid >= best_ask {
                return false;
            }
        }
        
        true
    }
}

impl fmt::Display for OrderBook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== HIGH-PERFORMANCE LOCK-FREE ORDER BOOK ===")?;
        
        let stats = self.get_stats();
        if let Some(spread) = stats.spread {
            writeln!(f, "Spread: {:.4}", spread)?;
        }
        if let Some(mid_price) = stats.mid_price {
            writeln!(f, "Mid Price: {:.4}", mid_price)?;
        }
        
        writeln!(f, "Total Orders: {}", self.get_total_orders())?;
        let (bid_levels, ask_levels) = self.get_total_price_levels();
        writeln!(f, "Price Levels - Bids: {}, Asks: {}", bid_levels, ask_levels)?;
        
        if let Some(last_match) = stats.last_match_time {
            writeln!(f, "Last Match: {}", last_match)?;
        }
        
        writeln!(f, "----------------")?;
        
        {
            let asks = self.asks.read();
            let ask_data: Vec<_> = asks.iter().rev().take(10)
                .map(|(price, level)| (price.as_f64(), level.get_total_quantity(), level.len()))
                .collect();
            
            for (price, quantity, order_count) in ask_data {
                writeln!(f, "ASK: {:.4} | {:.4} | {} orders", price, quantity, order_count)?;
            }
        }
        
        writeln!(f, "----------------")?;
        
        {
            let bids = self.bids.read();
            let bid_data: Vec<_> = bids.iter().rev().take(10)
                .map(|(price, level)| (price.as_f64(), level.get_total_quantity(), level.len()))
                .collect();
            
            for (price, quantity, order_count) in bid_data {
                writeln!(f, "BID: {:.4} | {:.4} | {} orders", price, quantity, order_count)?;
            }
        }
        
        writeln!(f, "----------------")?;
        writeln!(f, "Stats: Created: {}, Matched: {}, Cancelled: {}", 
            stats.total_orders_created, stats.total_orders_matched, stats.total_orders_cancelled)?;
        
        writeln!(f, "Consistency: {}", if self.validate_consistency() { "✅" } else { "❌" })?;
        
        Ok(())
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}
