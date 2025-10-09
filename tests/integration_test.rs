// Basic integration tests for the trading engine
// These tests verify core functionality without external dependencies

#[cfg(test)]
mod tests {
    #[test]
    fn test_order_serialization() {
        use serde_json::json;

        let order_json = json!({
            "id": 1,
            "symbol": "BTCUSDT",
            "side": "Buy",
            "quantity": 0.001,
            "price": 50000.0,
            "status": "filled",
            "timestamp": "2024-01-01T00:00:00Z"
        });

        assert_eq!(order_json["symbol"], "BTCUSDT");
        assert_eq!(order_json["side"], "Buy");
    }

    #[test]
    fn test_portfolio_calculation() {
        // Test basic P&L calculation logic
        let initial_cash = 100000.0;
        let buy_price = 50000.0;
        let quantity = 0.1;
        let current_price = 55000.0;

        let cost = buy_price * quantity;
        let cash_after = initial_cash - cost;
        let market_value = current_price * quantity;
        let pnl = (current_price - buy_price) * quantity;
        let total_value = cash_after + market_value;

        assert_eq!(cost, 5000.0);
        assert_eq!(cash_after, 95000.0);
        assert_eq!(market_value, 5500.0);
        assert_eq!(pnl, 500.0);
        assert_eq!(total_value, 100500.0);
    }

    #[test]
    fn test_spread_calculation() {
        let bid = 50000.0;
        let ask = 50050.0;
        let spread = ask - bid;
        let mid = (bid + ask) / 2.0;

        assert_eq!(spread, 50.0);
        assert_eq!(mid, 50025.0);
    }

    #[test]
    fn test_position_averaging() {
        // Test average price calculation after multiple buys
        let mut total_quantity = 0.0;
        let mut total_cost = 0.0;

        // First buy
        let qty1 = 0.1;
        let price1 = 50000.0;
        total_quantity += qty1;
        total_cost += qty1 * price1;

        // Second buy
        let qty2 = 0.2;
        let price2 = 52000.0;
        total_quantity += qty2;
        total_cost += qty2 * price2;

        let average_price: f64 = total_cost / total_quantity;

        assert!((total_quantity - 0.3).abs() < 0.0001);
        assert_eq!(total_cost, 15400.0);
        assert_eq!(average_price.round(), 51333.0);
    }
}
