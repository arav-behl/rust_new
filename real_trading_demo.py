#!/usr/bin/env python3
"""
Real Paper Trading System - No Mock Data
Connects to actual trading engine via REST API
"""

import streamlit as st
import requests
import pandas as pd
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import time
from datetime import datetime
import json

# Configure Streamlit
st.set_page_config(
    page_title="Real Paper Trading System",
    page_icon="🏦",
    layout="wide"
)

# API Configuration
API_BASE_URL = "http://localhost:8080"

# Custom CSS
st.markdown("""
<style>
    .main-header {
        font-size: 2.5rem;
        color: #00ff88;
        text-align: center;
        margin-bottom: 1rem;
        text-shadow: 0 0 10px rgba(0, 255, 136, 0.3);
    }
    .real-badge {
        background: linear-gradient(45deg, #ff6b35, #f7931e);
        color: white;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        font-weight: bold;
        display: inline-block;
        margin: 0.25rem;
    }
    .position-long {
        background: rgba(0, 255, 0, 0.1);
        color: #44ff44;
        padding: 0.5rem;
        border-radius: 5px;
        border-left: 4px solid #44ff44;
    }
    .position-short {
        background: rgba(255, 0, 0, 0.1);
        color: #ff4444;
        padding: 0.5rem;
        border-radius: 5px;
        border-left: 4px solid #ff4444;
    }
    .metric-box {
        background: linear-gradient(45deg, rgba(0, 255, 136, 0.1), rgba(0, 255, 136, 0.05));
        padding: 1rem;
        border-radius: 10px;
        border: 1px solid rgba(0, 255, 136, 0.3);
        margin: 0.5rem 0;
    }
</style>
""", unsafe_allow_html=True)

def check_api_health():
    """Check if the trading engine API is available"""
    try:
        response = requests.get(f"{API_BASE_URL}/health", timeout=2)
        if response.status_code == 200:
            data = response.json()
            return data.get('success', False), data.get('data', 'Unknown status')
        return False, "API returned error"
    except requests.exceptions.RequestException as e:
        return False, f"Connection failed: {str(e)}"

def get_portfolio():
    """Fetch real portfolio data from trading engine"""
    try:
        response = requests.get(f"{API_BASE_URL}/api/v1/portfolio", timeout=5)
        if response.status_code == 200:
            data = response.json()
            if data.get('success'):
                return data.get('data')
        return None
    except:
        return None

def get_orders():
    """Fetch real order history from trading engine"""
    try:
        response = requests.get(f"{API_BASE_URL}/api/v1/orders", timeout=5)
        if response.status_code == 200:
            data = response.json()
            if data.get('success'):
                return data.get('data', [])
        return []
    except:
        return []

def submit_order(symbol, side, quantity, price=None):
    """Submit real order to trading engine"""
    try:
        order_data = {
            "symbol": symbol,
            "side": side.lower(),
            "quantity": float(quantity)
        }
        if price is not None:
            order_data["price"] = float(price)

        response = requests.post(
            f"{API_BASE_URL}/api/v1/orders",
            json=order_data,
            timeout=5
        )

        if response.status_code == 200:
            data = response.json()
            return data.get('success', False), data.get('data'), data.get('error')

        return False, None, f"HTTP {response.status_code}"
    except Exception as e:
        return False, None, str(e)

# Main UI
st.markdown('<h1 class="main-header">🏦 Real Paper Trading System</h1>', unsafe_allow_html=True)

# Check API connectivity
api_healthy, api_status = check_api_health()

if api_healthy:
    st.markdown("""
    <div style="text-align: center;">
        <span class="real-badge">✅ REAL TRADING ENGINE CONNECTED</span>
        <span class="real-badge">🚫 NO MOCK DATA</span>
        <span class="real-badge">📊 LIVE POSITIONS</span>
    </div>
    """, unsafe_allow_html=True)
else:
    st.error(f"""
    🔴 **Trading Engine Offline**

    Status: {api_status}

    **To start the real trading engine:**
    1. Open terminal in the project directory
    2. Run: `cargo run --bin simple_engine`
    3. Wait for "REST API server starting on http://0.0.0.0:8080"
    4. Refresh this page
    """)
    st.stop()

# Create layout
col1, col2, col3 = st.columns([1, 1, 1])

# Portfolio Section
with col1:
    st.markdown("### 💼 Live Portfolio")

    portfolio = get_portfolio()
    if portfolio:
        st.metric("💰 Cash Balance", f"${portfolio.get('cash_balance', 0):,.2f}")
        st.metric("📈 Total Value", f"${portfolio.get('total_value', 0):,.2f}")
        st.metric("💹 Total P&L", f"${portfolio.get('total_pnl', 0):+,.2f}")

        # Show positions
        positions = portfolio.get('positions', {})
        if positions:
            st.markdown("**Active Positions:**")
            for symbol, position in positions.items():
                pnl = position.get('pnl', 0)
                quantity = position.get('quantity', 0)

                if quantity != 0:  # Only show non-zero positions
                    color_class = "position-long" if quantity > 0 else "position-short"
                    direction = "LONG" if quantity > 0 else "SHORT"

                    st.markdown(f"""
                    <div class="{color_class}">
                        <strong>{symbol}</strong> - {direction}<br>
                        Quantity: {abs(quantity):.4f}<br>
                        P&L: ${pnl:+.2f}
                    </div>
                    """, unsafe_allow_html=True)
        else:
            st.info("No active positions")
    else:
        st.warning("Could not load portfolio data")

# Trading Section
with col2:
    st.markdown("### 📋 Submit Real Order")

    with st.form("real_order_form"):
        symbol = st.selectbox("Symbol", ["BTCUSDT", "ETHUSDT", "ADAUSDT"])
        side = st.selectbox("Side", ["Buy", "Sell"])
        quantity = st.number_input(
            "Quantity",
            min_value=0.00001,
            value=0.1,
            step=0.1,
            format="%.5f"
        )

        order_type = st.selectbox("Order Type", ["Market", "Limit"])
        price = None
        if order_type == "Limit":
            price = st.number_input(
                "Limit Price",
                min_value=0.01,
                value=50000.0,
                step=100.0
            )

        submitted = st.form_submit_button("🚀 Submit Real Order")

        if submitted:
            with st.spinner("Executing order on real trading engine..."):
                success, order_data, error = submit_order(symbol, side, quantity, price)

                if success and order_data:
                    st.success(f"""
                    ✅ **Order Executed Successfully!**
                    - Order ID: {order_data.get('id', 'N/A')}
                    - Symbol: {order_data.get('symbol', 'N/A')}
                    - Side: {order_data.get('side', 'N/A').upper()}
                    - Quantity: {order_data.get('quantity', 0)}
                    - Status: {order_data.get('status', 'N/A').upper()}
                    - Time: {order_data.get('timestamp', 'N/A')}
                    """)
                else:
                    st.error(f"❌ Order Failed: {error}")

# Order History Section
with col3:
    st.markdown("### 📊 Real Order History")

    if st.button("🔄 Refresh Orders"):
        st.rerun()

    orders = get_orders()
    if orders:
        # Convert to DataFrame for display
        df = pd.DataFrame(orders)

        # Show recent orders
        if not df.empty:
            # Format timestamp if it exists
            if 'timestamp' in df.columns:
                df['time'] = pd.to_datetime(df['timestamp']).dt.strftime('%H:%M:%S')

            # Display columns we care about
            display_cols = ['time', 'symbol', 'side', 'quantity', 'status']
            available_cols = [col for col in display_cols if col in df.columns]

            if available_cols:
                st.dataframe(
                    df[available_cols].head(10),
                    use_container_width=True,
                    height=300
                )
            else:
                st.write("Order data structure:", df.columns.tolist())
                st.dataframe(df.head(5))
        else:
            st.info("No orders found")
    else:
        st.info("No order history available")

# System Status Footer
st.markdown("---")
st.markdown("### 🔧 System Status")

col_status1, col_status2, col_status3 = st.columns(3)

with col_status1:
    st.markdown("""
    **🏗️ Architecture**
    - Real paper trading engine
    - Persistent position tracking
    - Actual P&L calculations
    - REST API integration
    """)

with col_status2:
    st.markdown("""
    **🚫 Removed Mock Features**
    - No simulated latencies
    - No fake order processing
    - No generated portfolio data
    - No mock market data
    """)

with col_status3:
    st.markdown(f"""
    **📡 Live Connection**
    - API Status: {'🟢 Online' if api_healthy else '🔴 Offline'}
    - Endpoint: {API_BASE_URL}
    - Real-time updates: ✅
    - Mock data: ❌ Disabled
    """)

# Auto-refresh every 10 seconds to show live updates
time.sleep(0.1)
st.rerun()