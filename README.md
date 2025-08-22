# Order Book Trading Application

A high-performance, real-time order book trading application built in Rust with a terminal-based UI.

## Features

- **Real-time Order Book**: Live bid/ask order management with price-time priority
- **Multi-Exchange Support**: Binance and Polymarket integration
- **Advanced Trading**: Limit, market, and stop-loss orders
- **Technical Analysis**: Candlestick charts with moving averages
- **Price Alerts**: Configurable price and volume alerts
- **Multi-Coin Support**: BTC, ETH, SOL with real-time switching
- **Thread-Safe**: Lock-free concurrent order processing

## Architecture

### Core Components

```
src/
├── order_book.rs      # High-performance order book engine
├── order.rs           # Order and OrderSide definitions
├── price.rs           # Price handling with NaN safety
├── trade.rs           # Trade execution logic
├── binance_ws.rs      # Binance WebSocket client
├── binance_orders.rs  # Binance order management
├── polymarket_orders.rs # Polymarket CLOB integration
├── ui.rs              # Terminal UI with ratatui
├── main.rs            # Application entry point
└── lib.rs             # Module exports and tests
```

### Key Data Structures

- **OrderBook**: Thread-safe order book with atomic operations
- **OrderQueue**: Lock-free order queue using DashMap and SegQueue
- **PriceLevel**: Price level management with order aggregation
- **Order**: Order representation with side, price, quantity, timestamp

## Installation

### Prerequisites
- Rust 1.70+ 
- Terminal with Unicode support

### Build & Run
```bash
# Clone repository
git clone <repository-url>
cd order-book

# Build release version
cargo build --release

# Run application
cargo run --release
```

## Usage

### Navigation
- **Tabs**: `Tab` / `Shift+Tab` or `Left/Right` arrows
- **Quick Access**: `1-7` keys for direct tab selection
- **Function Keys**: `F2-F8` for instant tab switching

### Trading Interface
- **Order Input**: `P` or `Space` to activate order input mode
- **Order Side**: `B` for Buy, `S` for Sell
- **Order Type**: `G` (GTC), `F` (FOK), `D` (GTD)
- **Submit**: `Enter` to place orders

### Market Data
- **Refresh**: `R` to refresh order book
- **Sample Data**: `A` to add sample orders
- **Market Update**: `M` to update market data
- **Real Data**: `W` to toggle real/simulated data

### Coin Management
- **Switch**: `N` (next), `V` (previous)
- **Select**: `1-3` for specific coins (BTC, ETH, SOL)

### Charts & Analysis
- **Timeframes**: `<` / `>` to cycle through (1m, 5m, 15m, 1h, 4h, 1d)
- **Auto-refresh**: `L` to toggle automatic updates

### Price Alerts
```bash
# Command line alerts
alert above 27000 "BTC above 27k"
alert below 26000 "BTC below 26k"
alert change 5 "5% price change"
alert volume 1000000 "Volume spike"
alert cross 26500 "Price crosses 26.5k"
```

## Order Book Engine

### Performance Features
- **Lock-free Design**: Uses atomic operations and lock-free data structures
- **Concurrent Processing**: Multi-threaded order matching
- **Memory Efficient**: Optimized data structures with minimal allocations
- **Real-time Updates**: Sub-millisecond order processing

### Order Types
- **Limit Orders**: Good Till Cancelled (GTC), Good Till Date (GTD)
- **Market Orders**: Fill or Kill (FOK), Immediate or Cancel (IOC)
- **Stop Orders**: Stop-loss and take-profit orders

### Matching Engine
- **Price-Time Priority**: Orders matched by best price, then timestamp
- **Partial Fills**: Automatic order splitting for partial matches
- **Trade Generation**: Atomic trade execution with audit trail

## Exchange Integration

### Binance
- WebSocket real-time data streaming
- REST API order management
- Support for all major order types
- Authentication and signature generation

### Polymarket
- CLOB (Central Limit Order Book) integration
- ERC-1155 token support
- Multi-signature wallet integration
- Polygon chain compatibility

## Technical Specifications

### Performance Metrics
- **Order Processing**: <1ms latency
- **Concurrent Orders**: 10,000+ orders/second
- **Memory Usage**: <100MB for 1M orders
- **CPU Usage**: <5% on modern hardware

### Data Structures
- **BTreeMap**: Price level organization (O(log n) operations)
- **DashMap**: Concurrent order storage
- **SegQueue**: Lock-free FIFO order queue
- **AtomicU64**: Thread-safe counters

### Safety Features
- **NaN Handling**: Safe floating-point operations
- **Bounds Checking**: Comprehensive input validation
- **Error Handling**: Graceful failure with detailed logging
- **Memory Safety**: Rust's ownership system prevents data races

## Development

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_order_matching

# Run with output
cargo test -- --nocapture
```

### Code Quality
```bash
# Format code
cargo fmt

# Lint code
cargo clippy

# Check for security issues
cargo audit
```

### Performance Profiling
```bash
# Build with profiling
cargo build --release --features profiling

# Run benchmarks
cargo bench
```

## Configuration

### Environment Variables
```bash
# Binance API credentials
BINANCE_API_KEY=your_api_key
BINANCE_SECRET_KEY=your_secret_key

# Polymarket settings
POLYMARKET_HOST=https://clob.polymarket.com
POLYMARKET_CHAIN_ID=137
```

### Application Settings
- **Auto-refresh**: 2-second intervals
- **Chart timeframes**: 1m to 1d
- **Order book depth**: 20 levels by default
- **Alert system**: Configurable thresholds

## Contributing

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Support

- **Issues**: GitHub Issues
- **Documentation**: Code comments and this README
- **Community**: Rust trading community discussions

## Roadmap

- [ ] Additional exchange integrations
- [ ] Advanced charting indicators
- [ ] Backtesting framework
- [ ] Risk management tools
- [ ] API server for external access
- [ ] Mobile companion app
# order-book-rust
