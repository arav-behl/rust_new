# 🚀 Deployment Guide - Get Your Live Demo Link

This guide will help you deploy your Rust trading engine and get a **live, shareable URL** for your resume.

## Option 1: Railway (Recommended - Easiest)

Railway offers free tier hosting perfect for portfolio projects.

### Steps:

1. **Create Railway Account**
   - Go to [railway.app](https://railway.app)
   - Sign up with GitHub (free)

2. **Push Your Code to GitHub**
   ```bash
   cd /Users/aravbehl/ntu_college/y4/rust_crypto_project

   # Initialize git if not already done
   git init
   git add .
   git commit -m "Deploy trading engine to Railway"

   # Create repo on GitHub and push
   git remote add origin https://github.com/YOUR_USERNAME/rust-trading-engine.git
   git branch -M main
   git push -u origin main
   ```

3. **Deploy on Railway**
   - Go to [railway.app/new](https://railway.app/new)
   - Click "Deploy from GitHub repo"
   - Select your `rust-trading-engine` repository
   - Railway will auto-detect the Dockerfile and deploy!

4. **Configure Port**
   - After deployment, go to Settings → Networking
   - Click "Generate Domain"
   - Your app will be live at: `https://your-app-name.up.railway.app`

5. **Add to Resume**
   ```
   Live Demo: https://your-app-name.up.railway.app
   ```

**Time: ~5 minutes**

---

## Option 2: Render (Alternative - Also Free)

### Steps:

1. **Create Render Account**
   - Go to [render.com](https://render.com)
   - Sign up with GitHub (free)

2. **Push Code to GitHub** (same as above)

3. **Deploy on Render**
   - Go to [dashboard.render.com](https://dashboard.render.com)
   - Click "New +" → "Web Service"
   - Connect your GitHub repo
   - Configure:
     - Name: `rust-trading-engine`
     - Environment: `Docker`
     - Instance Type: `Free`
   - Click "Create Web Service"

4. **Get Your URL**
   - After deployment: `https://rust-trading-engine.onrender.com`

**Time: ~7 minutes**

---

## Option 3: Fly.io (For Tech-Savvy Users)

### Steps:

1. **Install Fly CLI**
   ```bash
   curl -L https://fly.io/install.sh | sh
   ```

2. **Sign Up and Login**
   ```bash
   fly auth signup  # or fly auth login
   ```

3. **Deploy**
   ```bash
   cd /Users/aravbehl/ntu_college/y4/rust_crypto_project
   fly launch --name rust-trading-engine
   # Follow prompts (choose defaults)
   fly deploy
   ```

4. **Get URL**
   ```bash
   fly status
   # Your URL: https://rust-trading-engine.fly.dev
   ```

**Time: ~10 minutes**

---

## Quick GitHub Setup (If You Haven't Already)

```bash
# Navigate to project
cd /Users/aravbehl/ntu_college/y4/rust_crypto_project

# Initialize git
git init

# Add all files
git add .

# Commit
git commit -m "Initial commit: Live cryptocurrency trading engine"

# Go to github.com and create new repo called "rust-trading-engine"
# Then run:
git remote add origin https://github.com/YOUR_USERNAME/rust-trading-engine.git
git branch -M main
git push -u origin main
```

---

## 📝 What to Put on Resume

Once deployed:

```
Live Cryptocurrency Trading Engine
Rust, Tokio, WebSocket, REST API | Live Demo: https://your-app.railway.app

• Built real-time trading engine processing live Binance WebSocket feeds
• Implemented concurrent state management with Arc/RwLock patterns
• Created REST API with Axum for order submission and portfolio tracking
• GitHub: github.com/YOUR_USERNAME/rust-trading-engine
```

---

## 🎯 Best Option: Railway

**Why Railway?**
- ✅ Easiest setup (auto-detects Dockerfile)
- ✅ Free tier includes 500 hours/month
- ✅ Custom domain generation
- ✅ Automatic HTTPS
- ✅ Zero config needed
- ✅ Great for portfolio projects

**Railway Free Tier:**
- 500 hours/month (plenty for demo)
- 512MB RAM
- 1GB storage
- Perfect for your use case

---

## ⚠️ Important Notes

### 1. Keep It Running
The free tiers may sleep after inactivity. That's OK! It'll wake up when someone visits (takes ~30 seconds).

### 2. Environment Variables
The app uses default settings. No special env vars needed for demo.

### 3. WebSocket Connectivity
Railway/Render/Fly all support WebSocket connections to Binance. No special config needed.

### 4. First Deployment
First build takes ~5-10 minutes (Rust compilation). Subsequent deployments are faster.

---

## 🚀 Quick Start Commands

### Railway (After pushing to GitHub):
```bash
# Install Railway CLI (optional - can use web UI)
npm i -g @railway/cli

# Login
railway login

# Link to project (if using CLI)
railway link

# Deploy
railway up
```

### Render:
Just use the web UI - it's the easiest.

### Fly.io:
```bash
fly launch
fly deploy
```

---

## 🎬 What Recruiters Will See

When they visit your link:

1. **Homepage** with live market data
   - Real-time prices for BTC/ETH/SOL updating
   - Order book spreads showing
   - Professional UI

2. **Working Demo**
   - They can submit test orders
   - See portfolio update in real-time
   - Verify the API works

3. **Proof It's Real**
   - Not just screenshots
   - Actually functional
   - Connected to real market data

---

## 📧 For Your Cover Letter

> "I've deployed a live demo at [your-url] where you can see the engine processing real-time market data from Binance and submit test orders through the REST API. The source code is available on GitHub at [your-github-link]."

---

## 🔧 Troubleshooting

**If deployment fails:**

1. Check logs in Railway/Render dashboard
2. Verify Dockerfile builds locally:
   ```bash
   docker build -t test .
   docker run -p 8080:8080 test
   ```
3. Most common issue: Port configuration
   - Railway/Render auto-detect port 8080
   - If needed, set `PORT` env var

**If app sleeps on free tier:**
- This is normal behavior
- Just note in resume: "Demo may take 30s to wake from sleep"
- Or use Railway's paid tier ($5/mo) for always-on

---

## ✅ Final Checklist

- [ ] Code pushed to GitHub (public repo)
- [ ] README.md looks good on GitHub
- [ ] Deployed to Railway/Render/Fly
- [ ] Tested the live URL (works in browser)
- [ ] Can submit orders via the UI
- [ ] Live URL added to resume
- [ ] GitHub link added to resume

---

**Estimated Total Time: 10-15 minutes from start to finish**

**Result: Live, shareable link you can send to recruiters! 🚀**
