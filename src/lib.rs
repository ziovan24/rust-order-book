pub mod order;
pub mod order_book;
pub mod price;
pub mod trade;
pub mod binance_ws;
pub mod polymarket_orders;
pub mod ui;

pub use order::{Order, OrderSide};
pub use order_book::OrderBook;
pub use price::Price;
pub use trade::Trade;
pub use binance_ws::run_binance_client;
pub use polymarket_orders::{PolymarketClobClient, PolymarketOrderSide, PolymarketOrderType, PolymarketSignatureType, PolymarketOrder, PolymarketOrderArgs};
pub use ui::App;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_creation() {
        let order_book = OrderBook::new();
        assert_eq!(order_book.get_total_orders(), 0);
        assert_eq!(order_book.get_total_price_levels(), (0, 0));
    }

    #[test]
    fn test_add_orders() {
        let order_book = OrderBook::new();
        
        let bid_id = order_book.add_order(OrderSide::Bid, 100.0, 10.0, 1);
        let ask_id = order_book.add_order(OrderSide::Ask, 101.0, 15.0, 2);
        
        assert_eq!(bid_id, 1);
        assert_eq!(ask_id, 2);
        assert_eq!(order_book.get_total_orders(), 2);
        assert_eq!(order_book.get_total_price_levels(), (1, 1));
    }

    #[test]
    fn test_order_matching() {
        let order_book = OrderBook::new();
        
        // Add crossing orders
        order_book.add_order(OrderSide::Bid, 100.0, 10.0, 1);
        order_book.add_order(OrderSide::Ask, 99.0, 15.0, 2);
        
        let trades = order_book.match_orders();
        assert_eq!(trades.len(), 1);
        
        let trade = &trades[0];
        assert_eq!(trade.price, 100.0); // Price-time priority: bid came first
        assert_eq!(trade.quantity, 10.0);
    }

    #[test]
    fn test_market_depth() {
        let order_book = OrderBook::new();
        
        order_book.add_order(OrderSide::Bid, 100.0, 10.0, 1);
        order_book.add_order(OrderSide::Bid, 99.0, 15.0, 2);
        order_book.add_order(OrderSide::Ask, 101.0, 20.0, 3);
        
        let (bids, asks) = order_book.get_market_depth(2);
        
        assert_eq!(bids.len(), 2);
        assert_eq!(asks.len(), 1);
        assert_eq!(bids[0].0, 100.0); // Highest bid first
        assert_eq!(bids[1].0, 99.0);
        assert_eq!(asks[0].0, 101.0);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;
        
        let order_book = Arc::new(OrderBook::new());
        let mut handles = vec![];
        
        // Spawn multiple threads to add orders concurrently
        for i in 0..10 {
            let order_book = Arc::clone(&order_book);
            let handle = thread::spawn(move || {
                order_book.add_order(OrderSide::Bid, 100.0 + i as f64, 10.0, i as u64);
                order_book.add_order(OrderSide::Ask, 101.0 + i as f64, 15.0, (i + 100) as u64);
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(order_book.get_total_orders(), 20);
        assert_eq!(order_book.get_total_price_levels(), (10, 10));
    }

    #[test]
    fn test_order_removal() {
        let order_book = OrderBook::new();
        
        let order_id = order_book.add_order(OrderSide::Bid, 100.0, 10.0, 1);
        assert_eq!(order_book.get_total_orders(), 1);
        
        let removed = order_book.remove_order(order_id);
        assert!(removed.is_some());
        assert_eq!(order_book.get_total_orders(), 0);
    }

    #[test]
    fn test_order_update() {
        let order_book = OrderBook::new();
        
        let order_id = order_book.add_order(OrderSide::Bid, 100.0, 10.0, 1);
        assert_eq!(order_book.get_total_orders(), 1);
        
        let updated = order_book.update_order(order_id, 15.0);
        assert!(updated);
        
        // Verify the update by checking market depth
        let (bids, _) = order_book.get_market_depth(1);
        assert_eq!(bids[0].1, 15.0);
    }

    #[test]
    fn test_statistics() {
        let order_book = OrderBook::new();
        
        order_book.add_order(OrderSide::Bid, 100.0, 10.0, 1);
        order_book.add_order(OrderSide::Ask, 101.0, 15.0, 2);
        
        let stats = order_book.get_stats();
        assert_eq!(stats.total_orders_created, 2);
        assert_eq!(stats.best_bid, Some(100.0));
        assert_eq!(stats.best_ask, Some(101.0));
        assert_eq!(stats.spread, Some(1.0));
        assert_eq!(stats.mid_price, Some(100.5));
    }
}
