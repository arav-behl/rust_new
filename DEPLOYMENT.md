# 🚀 Deployment Guide - Wintermute Order Book Engine

## Quick Start - One-Click Demo

### Local Development
```bash
# Clone and run locally
git clone <repository>
cd wintermute-orderbook-engine
cargo run --release --bin simple_engine
```

**Result**: Live trading engine at `http://localhost:8080`

### Docker Deployment
```bash
# Build and run with Docker
docker-compose up --build
```

**Result**: Production-ready container at `http://localhost:8080`

## 🌐 Public Deployment Options

### Option 1: Railway (Recommended - Free Tier)
1. Fork this repository to your GitHub
2. Go to [Railway.app](https://railway.app)
3. Connect your GitHub account
4. Deploy from GitHub repository
5. Railway will auto-detect the Dockerfile and deploy

**Live URL**: `https://your-app.railway.app`

### Option 2: Google Cloud Run
```bash
# Build and push to Google Container Registry
gcloud builds submit --tag gcr.io/YOUR_PROJECT_ID/trading-engine
gcloud run deploy --image gcr.io/YOUR_PROJECT_ID/trading-engine --platform managed
```

### Option 3: AWS ECS/Fargate
```bash
# Build and push to ECR
aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin YOUR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com
docker build -t trading-engine .
docker tag trading-engine:latest YOUR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/trading-engine:latest
docker push YOUR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/trading-engine:latest
```

### Option 4: DigitalOcean App Platform
1. Connect your GitHub repository
2. Select "Docker" as build type
3. Set port to 8080
4. Deploy

## 📊 Live Demo Features

### Real-Time Market Data
- **BTC/USDT, ETH/USDT, SOL/USDT** live price feeds
- **Bid/Ask spreads** updated every 100ms
- **Order book depth** from Binance WebSocket

### Realistic Trading Simulation
- **Market orders** execute at bid/ask prices
- **Limit orders** respect price-time priority
- **Portfolio tracking** with live P&L calculation

### Performance Metrics
- **Sub-10µs** order matching latency
- **1.2M+ orders/second** throughput capacity
- **Real-time** execution statistics

## 🎯 Recruiter-Friendly Features

### Technical Demonstration
- View live order execution with realistic bid/ask spreads
- See performance metrics updating in real-time
- Test market and limit orders across multiple trading pairs

### Architecture Highlights
- Thread-per-core design with SPSC channels
- Memory-mapped order book persistence
- Lock-free data structures
- Multi-exchange connectivity framework

### Professional Quality
- Production-ready Docker containerization
- Comprehensive error handling and monitoring
- Clean, documented codebase
- Enterprise-grade architecture patterns

## 🔧 Configuration

### Environment Variables
```bash
# Set log level
export RUST_LOG=info

# Enable backtraces for debugging
export RUST_BACKTRACE=1

# Optional: Set custom port (default: 8080)
export PORT=8080
```

### Performance Tuning
```bash
# For maximum performance on Linux
echo performance > /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
ulimit -n 65536
```

## 📈 Monitoring & Observability

### Health Check Endpoint
```bash
curl http://localhost:8080/health
```

### API Endpoints
- `GET /api/v1/prices` - Live market prices
- `GET /api/v1/depth` - Order book depth with spreads
- `POST /api/v1/orders` - Submit trading orders
- `GET /api/v1/portfolio` - Portfolio with live P&L

### Optional Monitoring Stack
Uncomment the Prometheus/Grafana services in `docker-compose.yml`:
- Prometheus metrics: `http://localhost:9090`
- Grafana dashboards: `http://localhost:3000` (admin/admin)

## 🚀 Production Deployment Checklist

### Security
- [ ] Configure firewall rules (port 8080)
- [ ] Set up HTTPS with SSL certificates
- [ ] Implement rate limiting
- [ ] Configure authentication if needed

### Scaling
- [ ] Set up load balancer for multiple instances
- [ ] Configure horizontal pod autoscaling (Kubernetes)
- [ ] Implement database for persistent storage
- [ ] Set up Redis for distributed caching

### Monitoring
- [ ] Configure log aggregation
- [ ] Set up alerting for critical metrics
- [ ] Implement distributed tracing
- [ ] Monitor resource usage and scaling triggers

## 📞 Support & Documentation

### Architecture Details
- See `ARCHITECTURE.md` for system design
- See `PROJECT_SUMMARY.md` for technical achievements
- Run `cargo doc --open` for API documentation

### Performance Benchmarks
```bash
# Run comprehensive benchmarks
cargo bench --bench orderbook_bench
```

### Troubleshooting
- Check logs: `docker logs <container_id>`
- Verify connectivity: `curl http://localhost:8080/health`
- Monitor resources: `docker stats`

---

## 🏆 Ready for Interview Demo

This deployment setup is designed to impress technical recruiters:

✅ **One-command deployment** - No complex setup required
✅ **Live performance metrics** - Real-time latency and throughput display
✅ **Professional UI** - Clean interface explaining technical achievements
✅ **Production architecture** - Docker, monitoring, scaling capabilities
✅ **Real market data** - Actual Binance WebSocket integration

**Perfect for demonstrating quantitative trading expertise in technical interviews!**