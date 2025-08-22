use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BinanceOrderSide {
    BUY,
    SELL,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BinanceOrderType {
    LIMIT,
    MARKET,
    STOP_LOSS,
    TAKE_PROFIT,
    STOP_MARKET,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BinanceTimeInForce {
    GTC,
    IOC,
    FOK,
    GTX,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinanceOrderRequest {
    pub symbol: String,
    pub side: BinanceOrderSide,
    pub order_type: BinanceOrderType,
    pub time_in_force: Option<BinanceTimeInForce>,
    pub quantity: f64,
    pub price: Option<f64>,
    pub stop_price: Option<f64>,
    pub iceberg_qty: Option<f64>,
    pub new_client_order_id: Option<String>,
    pub new_order_resp_type: Option<String>,
    pub recv_window: Option<u64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOrderResponse {
    pub symbol: String,
    pub order_id: u64,
    pub order_list_id: Option<i64>,
    pub client_order_id: String,
    pub transact_time: u64,
    pub price: String,
    pub orig_qty: String,
    pub executed_qty: String,
    pub cummulative_quote_qty: String,
    pub status: BinanceOrderStatus,
    pub time_in_force: String,
    pub order_type: String,
    pub side: String,
    pub fills: Option<Vec<BinanceFill>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum BinanceOrderStatus {
    NEW,
    PARTIALLY_FILLED,
    FILLED,
    CANCELED,
    PENDING_CANCEL,
    REJECTED,
    EXPIRED,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceFill {
    pub price: String,
    pub qty: String,
    pub commission: String,
    pub commission_asset: String,
    pub trade_id: u64,
}

pub struct BinanceOrderClient {
    api_key: String,
    secret_key: String,
    base_url: String,
    recv_window: u64,
}

impl BinanceOrderClient {
    pub fn new(api_key: String, secret_key: String, testnet: bool) -> Self {
        let base_url = if testnet {
            "https://testnet.binancefuture.com".to_string()
        } else {
            "https://fapi.binance.com".to_string()
        };

        Self {
            api_key,
            secret_key,
            base_url,
            recv_window: 5000,
        }
    }

    pub fn create_limit_order(
        &self,
        symbol: &str,
        side: BinanceOrderSide,
        quantity: f64,
        price: f64,
        time_in_force: BinanceTimeInForce,
    ) -> BinanceOrderRequest {
        BinanceOrderRequest {
            symbol: symbol.to_string(),
            side,
            order_type: BinanceOrderType::LIMIT,
            time_in_force: Some(time_in_force),
            quantity,
            price: Some(price),
            stop_price: None,
            iceberg_qty: None,
            new_client_order_id: None,
            new_order_resp_type: Some("RESULT".to_string()),
            recv_window: Some(self.recv_window),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    pub fn create_market_order(
        &self,
        symbol: &str,
        side: BinanceOrderSide,
        quantity: f64,
    ) -> BinanceOrderRequest {
        BinanceOrderRequest {
            symbol: symbol.to_string(),
            side,
            order_type: BinanceOrderType::MARKET,
            time_in_force: None,
            quantity,
            price: None,
            stop_price: None,
            iceberg_qty: None,
            new_client_order_id: None,
            new_order_resp_type: Some("RESULT".to_string()),
            recv_window: Some(self.recv_window),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    pub fn create_stop_loss_order(
        &self,
        symbol: &str,
        side: BinanceOrderSide,
        quantity: f64,
        stop_price: f64,
        price: Option<f64>,
    ) -> BinanceOrderRequest {
        let order_type = if price.is_some() {
            BinanceOrderType::STOP_LOSS
        } else {
            BinanceOrderType::STOP_MARKET
        };

        BinanceOrderRequest {
            symbol: symbol.to_string(),
            side,
            order_type,
            time_in_force: None,
            quantity,
            price,
            stop_price: Some(stop_price),
            iceberg_qty: None,
            new_client_order_id: None,
            new_order_resp_type: Some("RESULT".to_string()),
            recv_window: Some(self.recv_window),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    pub fn convert_polymarket_order_type(polymarket_type: &str) -> (BinanceOrderType, Option<BinanceTimeInForce>) {
        match polymarket_type {
            "GTC" => (BinanceOrderType::LIMIT, Some(BinanceTimeInForce::GTC)),
            "FOK" => (BinanceOrderType::MARKET, None),
            "FAK" => (BinanceOrderType::MARKET, None),
            "GTD" => (BinanceOrderType::LIMIT, Some(BinanceTimeInForce::GTC)),
            _ => (BinanceOrderType::LIMIT, Some(BinanceTimeInForce::GTC)),
        }
    }

    pub fn generate_signature(&self, query_string: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub fn build_query_string(&self, order: &BinanceOrderRequest) -> String {
        let mut params = HashMap::new();
        
        params.insert("symbol", order.symbol.clone());
        params.insert("side", format!("{:?}", order.side));
        params.insert("type", format!("{:?}", order.order_type));
        
        if let Some(time_in_force) = &order.time_in_force {
            params.insert("timeInForce", format!("{:?}", time_in_force));
        }
        
        params.insert("quantity", order.quantity.to_string());
        
        if let Some(price) = order.price {
            params.insert("price", price.to_string());
        }
        
        if let Some(stop_price) = order.stop_price {
            params.insert("stopPrice", stop_price.to_string());
        }
        
        if let Some(iceberg_qty) = order.iceberg_qty {
            params.insert("icebergQty", iceberg_qty.to_string());
        }
        
        if let Some(client_order_id) = &order.new_client_order_id {
            params.insert("newClientOrderId", client_order_id.clone());
        }
        
        if let Some(resp_type) = &order.new_order_resp_type {
            params.insert("newOrderRespType", resp_type.clone());
        }
        
        params.insert("recvWindow", order.recv_window.unwrap_or(5000).to_string());
        params.insert("timestamp", order.timestamp.to_string());
        
        params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    }
}

pub fn convert_polymarket_to_binance_example() {
    let polymarket_order = PolymarketOrderArgs {
        price: 0.01,
        size: 5.0,
        side: PolymarketOrderSide::BUY,
        token_id: "12345".to_string(),
    };

    let binance_client = BinanceOrderClient::new(
        "your_api_key".to_string(),
        "your_secret_key".to_string(),
        true,
    );

    let binance_order = binance_client.create_limit_order(
        "BTCUSDT",
        BinanceOrderSide::BUY,
        polymarket_order.size,
        polymarket_order.price,
        BinanceTimeInForce::GTC,
    );

    println!("Converted Polymarket order to Binance: {:?}", binance_order);
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderArgs {
    pub price: f64,
    pub size: f64,
    pub side: PolymarketOrderSide,
    pub token_id: String,
}

#[derive(Debug, Clone)]
pub enum PolymarketOrderSide {
    BUY,
    SELL,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_type_conversion() {
        let (order_type, time_in_force) = BinanceOrderClient::convert_polymarket_order_type("GTC");
        assert_eq!(order_type, BinanceOrderType::LIMIT);
        assert_eq!(time_in_force, Some(BinanceTimeInForce::GTC));

        let (order_type, time_in_force) = BinanceOrderClient::convert_polymarket_order_type("FOK");
        assert_eq!(order_type, BinanceOrderType::MARKET);
        assert_eq!(time_in_force, None);
    }

    #[test]
    fn test_limit_order_creation() {
        let client = BinanceOrderClient::new(
            "test_key".to_string(),
            "test_secret".to_string(),
            true,
        );

        let order = client.create_limit_order(
            "BTCUSDT",
            BinanceOrderSide::BUY,
            1.0,
            50000.0,
            BinanceTimeInForce::GTC,
        );

        assert_eq!(order.symbol, "BTCUSDT");
        assert_eq!(order.side, BinanceOrderSide::BUY);
        assert_eq!(order.order_type, BinanceOrderType::LIMIT);
        assert_eq!(order.quantity, 1.0);
        assert_eq!(order.price, Some(50000.0));
    }

    #[test]
    fn test_query_string_building() {
        let client = BinanceOrderClient::new(
            "test_key".to_string(),
            "test_secret".to_string(),
            true,
        );

        let order = client.create_limit_order(
            "BTCUSDT",
            BinanceOrderSide::BUY,
            1.0,
            50000.0,
            BinanceTimeInForce::GTC,
        );

        let query_string = client.build_query_string(&order);
        assert!(query_string.contains("symbol=BTCUSDT"));
        assert!(query_string.contains("side=BUY"));
        assert!(query_string.contains("type=LIMIT"));
        assert!(query_string.contains("quantity=1"));
        assert!(query_string.contains("price=50000"));
    }
}
