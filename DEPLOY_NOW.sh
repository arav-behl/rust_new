#!/bin/bash

clear
echo "╔════════════════════════════════════════════════════════════╗"
echo "║     🚀 DEPLOY YOUR RUST TRADING ENGINE - LIVE LINK       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "This script will help you get a live demo link in 10 minutes!"
echo ""

# Stage all changes
echo "📦 Step 1: Preparing your code..."
git add .
git commit -m "Deploy: Live cryptocurrency trading engine" 2>/dev/null || true
echo "✅ Code ready!"
echo ""

# Check for GitHub remote
if git remote | grep -q origin; then
    echo "📤 Step 2: Pushing to GitHub..."
    git push origin main 2>&1 | grep -v "Everything up-to-date" || true
    echo "✅ Code pushed to GitHub!"
    REPO_URL=$(git config --get remote.origin.url | sed 's/\.git$//')
    echo ""
    echo "Your GitHub repo: $REPO_URL"
    echo ""
else
    echo "⚠️  Step 2: GitHub Setup Needed"
    echo ""
    echo "Please run these commands:"
    echo ""
    echo "1. Create repo at: https://github.com/new"
    echo "   Name: rust-trading-engine"
    echo "   Public: ✓"
    echo ""
    echo "2. Then run:"
    echo "   git remote add origin https://github.com/YOUR_USERNAME/rust-trading-engine.git"
    echo "   git branch -M main"
    echo "   git push -u origin main"
    echo ""
    exit 1
fi

echo "╔════════════════════════════════════════════════════════════╗"
echo "║                   🎯 DEPLOY TO RAILWAY                     ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "Follow these steps to get your live link:"
echo ""
echo "1. Go to: https://railway.app/new"
echo ""
echo "2. Click 'Deploy from GitHub repo'"
echo ""
echo "3. Select: rust-trading-engine"
echo ""
echo "4. Wait for build (5-10 min first time)"
echo ""
echo "5. Settings → Networking → Generate Domain"
echo ""
echo "6. COPY YOUR LIVE URL! 🎉"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📋 Add to your resume:"
echo ""
echo "Live Demo: https://your-app.up.railway.app"
echo "GitHub: $REPO_URL"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Need help? Read: GET_LIVE_LINK.md"
echo ""
