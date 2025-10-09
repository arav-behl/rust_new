// Professional Trading Interface JavaScript
class TradingDashboard {
    constructor() {
        this.apiBase = '';
        this.prices = {};
        this.previousPrices = {};
        this.updateInterval = null;
        this.connectionStatus = false;

        this.init();
    }

    async init() {
        this.setupEventListeners();
        await this.checkConnection();
        this.startDataUpdates();
    }

    setupEventListeners() {
        // Order form submission
        const orderForm = document.getElementById('orderForm');
        orderForm.addEventListener('submit', (e) => this.handleOrderSubmission(e));

        // Symbol selection updates price in form
        const symbolSelect = document.getElementById('symbol');
        symbolSelect.addEventListener('change', () => this.updateOrderFormPrice());
    }

    async checkConnection() {
        try {
            const response = await fetch(`${this.apiBase}/health`);
            const data = await response.json();

            if (data.success) {
                this.updateConnectionStatus(true);
                console.log('Connected to trading engine');
            } else {
                this.updateConnectionStatus(false);
            }
        } catch (error) {
            console.error('Connection failed:', error);
            this.updateConnectionStatus(false);
        }
    }

    updateConnectionStatus(connected) {
        this.connectionStatus = connected;
        const statusDot = document.getElementById('statusDot');
        const statusText = document.getElementById('connectionStatus');

        if (connected) {
            statusDot.classList.add('connected');
            statusText.textContent = 'Connected - Live Data';
        } else {
            statusDot.classList.remove('connected');
            statusText.textContent = 'Disconnected';
        }
    }

    async startDataUpdates() {
        // Initial load
        await this.updatePrices();
        await this.updatePortfolio();
        await this.updateOrders();

        // Set up regular updates
        this.updateInterval = setInterval(async () => {
            await this.updatePrices();
            await this.updatePortfolio();
        }, 2000); // Update every 2 seconds
    }

    async updatePrices() {
        try {
            const response = await fetch(`${this.apiBase}/api/v1/prices`);
            const data = await response.json();

            if (data.success && data.data) {
                this.previousPrices = { ...this.prices };
                this.prices = data.data;
                this.renderPrices();
            }
        } catch (error) {
            console.error('Failed to fetch prices:', error);
        }
    }

    renderPrices() {
        Object.entries(this.prices).forEach(([symbol, price]) => {
            const priceElement = document.getElementById(`price-${symbol}`);
            const changeElement = document.getElementById(`change-${symbol}`);

            if (priceElement) {
                // Update price with animation
                priceElement.textContent = `$${this.formatPrice(price)}`;

                // Show price change indication
                const previousPrice = this.previousPrices[symbol];
                if (previousPrice) {
                    const change = price - previousPrice;
                    const changePercent = ((change / previousPrice) * 100).toFixed(2);

                    if (changeElement) {
                        changeElement.textContent = `${change >= 0 ? '+' : ''}${changePercent}%`;
                        changeElement.className = `change ${change >= 0 ? 'positive' : 'negative'}`;
                    }

                    // Flash animation for price changes
                    if (change !== 0) {
                        priceElement.style.backgroundColor = change > 0 ? '#00d4aa20' : '#ff474720';
                        setTimeout(() => {
                            priceElement.style.backgroundColor = '';
                        }, 500);
                    }
                }
            }
        });
    }

    async updatePortfolio() {
        try {
            const response = await fetch(`${this.apiBase}/api/v1/portfolio`);
            const data = await response.json();

            if (data.success && data.data) {
                this.renderPortfolio(data.data);
            }
        } catch (error) {
            console.error('Failed to fetch portfolio:', error);
        }
    }

    renderPortfolio(portfolio) {
        // Update summary
        document.getElementById('totalValue').textContent = `$${this.formatCurrency(portfolio.total_value)}`;
        document.getElementById('cashBalance').textContent = `$${this.formatCurrency(portfolio.cash_balance)}`;

        const pnlElement = document.getElementById('totalPnL');
        pnlElement.textContent = `$${this.formatCurrency(portfolio.total_pnl)}`;
        pnlElement.className = `value ${portfolio.total_pnl >= 0 ? 'positive' : 'negative'}`;

        // Update positions
        const positionsContainer = document.getElementById('positions');
        if (Object.keys(portfolio.positions).length === 0) {
            positionsContainer.innerHTML = '<div style="color: var(--text-muted); text-align: center; padding: 1rem;">No positions</div>';
        } else {
            positionsContainer.innerHTML = Object.entries(portfolio.positions)
                .map(([symbol, position]) => `
                    <div class="position-item">
                        <div class="symbol">${symbol}</div>
                        <div>${this.formatNumber(position.quantity)}</div>
                        <div>$${this.formatPrice(position.average_price)}</div>
                        <div class="pnl ${position.pnl >= 0 ? 'positive' : 'negative'}">
                            $${this.formatCurrency(position.pnl)}
                        </div>
                    </div>
                `).join('');
        }
    }

    async updateOrders() {
        try {
            const response = await fetch(`${this.apiBase}/api/v1/orders`);
            const data = await response.json();

            if (data.success && data.data) {
                this.renderOrders(data.data);
            }
        } catch (error) {
            console.error('Failed to fetch orders:', error);
        }
    }

    renderOrders(orders) {
        const ordersBody = document.getElementById('ordersBody');

        if (orders.length === 0) {
            ordersBody.innerHTML = '<div style="grid-column: 1/-1; color: var(--text-muted); text-align: center; padding: 1rem;">No orders</div>';
        } else {
            ordersBody.innerHTML = orders
                .sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp))
                .slice(0, 10) // Show last 10 orders
                .map(order => `
                    <div class="order-row">
                        <div>${this.formatTime(order.timestamp)}</div>
                        <div>${order.symbol}</div>
                        <div class="side ${order.side}">${order.side.toUpperCase()}</div>
                        <div>${this.formatNumber(order.quantity)}</div>
                        <div>$${this.formatPrice(order.price || 0)}</div>
                        <div class="status ${order.status}">${order.status.toUpperCase()}</div>
                    </div>
                `).join('');
        }
    }

    async handleOrderSubmission(e) {
        e.preventDefault();

        const formData = new FormData(e.target);
        const orderData = {
            symbol: formData.get('symbol'),
            side: formData.get('side'),
            quantity: parseFloat(formData.get('quantity')),
            price: formData.get('price') ? parseFloat(formData.get('price')) : null
        };

        // Validate order
        if (!orderData.quantity || orderData.quantity <= 0) {
            this.showNotification('Please enter a valid quantity', 'error');
            return;
        }

        try {
            const submitBtn = document.querySelector('.submit-btn');
            submitBtn.disabled = true;
            submitBtn.textContent = 'Placing Order...';

            const response = await fetch(`${this.apiBase}/api/v1/orders`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(orderData)
            });

            const result = await response.json();

            if (result.success) {
                this.showNotification('Order placed successfully', 'success');
                e.target.reset();

                // Update orders immediately
                await this.updateOrders();
                await this.updatePortfolio();
            } else {
                this.showNotification(result.error || 'Failed to place order', 'error');
            }
        } catch (error) {
            console.error('Order submission failed:', error);
            this.showNotification('Failed to place order', 'error');
        } finally {
            const submitBtn = document.querySelector('.submit-btn');
            submitBtn.disabled = false;
            submitBtn.textContent = 'Place Order';
        }
    }

    updateOrderFormPrice() {
        const symbol = document.getElementById('symbol').value;
        const priceInput = document.getElementById('price');

        if (this.prices[symbol]) {
            priceInput.placeholder = `Market: $${this.formatPrice(this.prices[symbol])}`;
        }
    }

    showNotification(message, type = 'info') {
        const notification = document.createElement('div');
        notification.className = `notification ${type}`;
        notification.textContent = message;

        document.body.appendChild(notification);

        setTimeout(() => {
            notification.remove();
        }, 4000);
    }

    formatPrice(price) {
        return parseFloat(price).toLocaleString('en-US', {
            minimumFractionDigits: 2,
            maximumFractionDigits: 6
        });
    }

    formatCurrency(amount) {
        return parseFloat(amount).toLocaleString('en-US', {
            minimumFractionDigits: 2,
            maximumFractionDigits: 2
        });
    }

    formatNumber(num) {
        return parseFloat(num).toLocaleString('en-US', {
            minimumFractionDigits: 0,
            maximumFractionDigits: 8
        });
    }

    formatTime(timestamp) {
        return new Date(timestamp).toLocaleTimeString('en-US', {
            hour12: false,
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });
    }

    destroy() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
        }
    }
}

// Initialize the trading dashboard when the page loads
document.addEventListener('DOMContentLoaded', () => {
    window.tradingDashboard = new TradingDashboard();
});

// Handle page visibility changes to pause/resume updates
document.addEventListener('visibilitychange', () => {
    if (window.tradingDashboard) {
        if (document.hidden) {
            if (window.tradingDashboard.updateInterval) {
                clearInterval(window.tradingDashboard.updateInterval);
            }
        } else {
            window.tradingDashboard.startDataUpdates();
        }
    }
});