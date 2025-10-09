# 🚀 Get Your Live Demo Link in 5 Minutes

Follow these exact steps to get a shareable link for recruiters.

## Step 1: Push to GitHub (2 minutes)

```bash
# Navigate to your project
cd /Users/aravbehl/ntu_college/y4/rust_crypto_project

# Initialize git (if not already done)
git init
git add .
git commit -m "Deploy: Live cryptocurrency trading engine"
```

**Now go to GitHub:**
1. Visit [github.com/new](https://github.com/new)
2. Repository name: `rust-trading-engine`
3. Make it **Public** (important!)
4. Click "Create repository"

**Back in terminal:**
```bash
# Replace YOUR_USERNAME with your actual GitHub username
git remote add origin https://github.com/YOUR_USERNAME/rust-trading-engine.git
git branch -M main
git push -u origin main
```

✅ **Your code is now on GitHub!**

---

## Step 2: Deploy to Railway (3 minutes)

1. **Go to Railway**
   - Visit: [railway.app](https://railway.app)
   - Click "Start a New Project"
   - Login with GitHub (it's free!)

2. **Deploy Your Repo**
   - Click "Deploy from GitHub repo"
   - Select `rust-trading-engine`
   - Railway automatically detects Dockerfile and starts deploying!

3. **Wait for Build** (5-10 minutes first time)
   - You'll see build logs
   - Wait for "Success" message
   - Future deployments are faster

4. **Get Your Live URL**
   - Go to "Settings" tab
   - Click "Networking" or "Domains"
   - Click "Generate Domain"
   - **Copy your URL**: `https://rust-trading-engine-production-XXXX.up.railway.app`

✅ **Your app is LIVE!**

---

## Step 3: Test Your Live Link

1. Open your Railway URL in a browser
2. You should see:
   - Live market data updating (BTC, ETH, SOL)
   - Order submission form
   - Real-time prices from Binance

3. Submit a test order to verify it works

✅ **Your demo is working!**

---

## Step 4: Add to Resume

```
Live Cryptocurrency Trading Engine
Rust, Tokio, WebSocket, REST API

• Built real-time trading engine processing live Binance WebSocket feeds
• Implemented concurrent state management with Arc/RwLock patterns
• Created REST API with Axum for order submission and portfolio tracking

🔗 Live Demo: https://your-app.up.railway.app
📦 GitHub: github.com/YOUR_USERNAME/rust-trading-engine
```

---

## 🎯 What Recruiters Will See

When they click your link:
- ✅ Real application running (not just screenshots)
- ✅ Live market data updating
- ✅ Working order submission
- ✅ Professional UI
- ✅ Proof you can build and deploy

---

## 💡 Pro Tips

**1. Note on Free Tier**
Railway free tier may sleep after inactivity. Add to resume:
> "Live demo (may take 30s to wake from sleep on free tier)"

**2. Keep It Running**
Visit your own link once a day to keep it warm.

**3. Railway Dashboard**
- View logs: See WebSocket connections in real-time
- Monitor: Check memory/CPU usage
- Redeploy: Push to GitHub = auto-redeploy

**4. If Build Fails**
Check Railway logs for errors. Most common:
- Build timeout (first build takes longest)
- Port issues (Railway auto-detects 8080, should work fine)

---

## Alternative: Render (If Railway Doesn't Work)

1. Go to [render.com](https://render.com)
2. Sign up with GitHub
3. New → Web Service
4. Connect your repo
5. Settings:
   - Environment: `Docker`
   - Instance Type: `Free`
6. Create Web Service
7. Wait 10 minutes for first deploy
8. Get URL: `https://rust-trading-engine.onrender.com`

---

## ✅ Success Checklist

- [ ] Code on GitHub (public repo)
- [ ] Deployed to Railway or Render
- [ ] Tested live URL (loads in browser)
- [ ] Can submit orders via UI
- [ ] Added live link to resume
- [ ] Added GitHub link to resume

---

## 🆘 Need Help?

**Build failing?**
- Check Dockerfile builds locally: `docker build -t test .`
- Check Railway/Render logs for specific error

**App not loading?**
- Wait 30 seconds (free tier wakes from sleep)
- Check Railway/Render dashboard shows "Active"
- Verify port 8080 is exposed

**WebSocket not connecting?**
- Check browser console for errors
- Railway/Render support WSS by default, should work

---

**Total Time: ~10 minutes from start to working demo link! 🚀**
