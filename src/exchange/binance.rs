use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::orderbook::SharedOrderBook;

/// Binance ticker message structure
#[derive(Debug, Deserialize)]
struct BinanceTicker {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    price: String,
}

/// Binance depth update structure
#[derive(Debug, Deserialize)]
struct BinanceDepth {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "b")]
    bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    asks: Vec<[String; 2]>,
}

/// Market data snapshot for a symbol
#[derive(Debug, Clone)]
pub struct MarketData {
    pub symbol: String,
    pub price: f64,
    pub bid_price: f64,
    pub ask_price: f64,
    pub spread: f64,
}

/// Binance WebSocket feed manager
pub struct BinanceFeed {
    symbols: Vec<String>,
    market_data: Arc<RwLock<Vec<MarketData>>>,
}

impl BinanceFeed {
    pub fn new(symbols: Vec<String>) -> Self {
        Self {
            symbols,
            market_data: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start the price feed (ticker stream)
    pub async fn start_price_feed(&self) {
        let stream_names: Vec<String> = self
            .symbols
            .iter()
            .map(|s| format!("{}@ticker", s.to_lowercase()))
            .collect();

        let url = format!(
            "wss://stream.binance.com:9443/ws/{}",
            stream_names.join("/")
        );

        let market_data = Arc::clone(&self.market_data);

        tokio::spawn(async move {
            loop {
                match connect_async(&url).await {
                    Ok((ws_stream, _)) => {
                        tracing::info!("✓ Connected to Binance ticker feed");
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                // Direct parsing without wrapper
                                if let Ok(ticker) = serde_json::from_str::<BinanceTicker>(&text) {
                                    if let Ok(price) = ticker.price.parse::<f64>() {
                                        tracing::info!("📊 {} = ${:.2}", ticker.symbol, price);

                                        // Update market data
                                        let mut data = market_data.write().await;
                                        if let Some(md) = data.iter_mut().find(|m| m.symbol == ticker.symbol) {
                                            md.price = price;
                                        } else {
                                            data.push(MarketData {
                                                symbol: ticker.symbol,
                                                price,
                                                bid_price: 0.0,
                                                ask_price: 0.0,
                                                spread: 0.0,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Connection failed: {}", e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
    }

    /// Start the depth feed (order book updates)
    pub async fn start_depth_feed(&self, _orderbook: SharedOrderBook) {
        let stream_names: Vec<String> = self
            .symbols
            .iter()
            .map(|s| format!("{}@depth5@100ms", s.to_lowercase()))
            .collect();

        let url = format!(
            "wss://stream.binance.com:9443/ws/{}",
            stream_names.join("/")
        );

        let market_data = Arc::clone(&self.market_data);

        tokio::spawn(async move {
            loop {
                match connect_async(&url).await {
                    Ok((ws_stream, _)) => {
                        tracing::info!("✓ Connected to Binance depth feed");
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                // Direct parsing without wrapper
                                if let Ok(depth) = serde_json::from_str::<BinanceDepth>(&text) {
                                    // Update market data with best bid/ask
                                    if let (Some(best_bid), Some(best_ask)) =
                                        (depth.bids.first(), depth.asks.first()) {

                                        if let (Ok(bid_price), Ok(ask_price)) =
                                            (best_bid[0].parse::<f64>(), best_ask[0].parse::<f64>()) {

                                            let spread = ask_price - bid_price;

                                            // Update market data
                                            let mut data = market_data.write().await;
                                            if let Some(md) = data.iter_mut().find(|m| m.symbol == depth.symbol) {
                                                md.bid_price = bid_price;
                                                md.ask_price = ask_price;
                                                md.spread = spread;
                                            }

                                            tracing::debug!(
                                                "📖 {} Bid: ${:.2} Ask: ${:.2} Spread: ${:.2}",
                                                depth.symbol, bid_price, ask_price, spread
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Depth connection failed: {}", e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
    }

    /// Get current market data snapshot
    pub async fn get_market_data(&self) -> Vec<MarketData> {
        self.market_data.read().await.clone()
    }

    /// Get market data for a specific symbol
    pub async fn get_symbol_data(&self, symbol: &str) -> Option<MarketData> {
        self.market_data
            .read()
            .await
            .iter()
            .find(|md| md.symbol == symbol)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_creation() {
        let feed = BinanceFeed::new(vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        assert_eq!(feed.symbols.len(), 2);
    }
}
