#!/bin/bash
# Quick deployment script for Railway

echo "🚀 Deploying Rust Trading Engine to Railway"
echo "============================================"
echo ""

# Check if git is initialized
if [ ! -d .git ]; then
    echo "📦 Initializing git repository..."
    git init
    git add .
    git commit -m "Initial commit: Live cryptocurrency trading engine"
    echo "✅ Git initialized"
    echo ""
fi

# Check if remote is set
if ! git remote | grep -q origin; then
    echo "⚠️  No git remote found."
    echo ""
    echo "Please create a GitHub repository and run:"
    echo "  git remote add origin https://github.com/YOUR_USERNAME/rust-trading-engine.git"
    echo "  git branch -M main"
    echo "  git push -u origin main"
    echo ""
    echo "Then visit https://railway.app/new to deploy from GitHub"
    exit 1
fi

# Push to GitHub
echo "📤 Pushing to GitHub..."
git add .
git commit -m "Update for deployment" || echo "No changes to commit"
git push origin main

echo ""
echo "✅ Code pushed to GitHub!"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Next Steps:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "1. Go to: https://railway.app/new"
echo "2. Click 'Deploy from GitHub repo'"
echo "3. Select your repository"
echo "4. Railway will auto-deploy!"
echo ""
echo "After deployment:"
echo "• Go to Settings → Networking"
echo "• Click 'Generate Domain'"
echo "• Your live URL will be ready!"
echo ""
echo "Add to your resume:"
echo "  Live Demo: https://your-app.up.railway.app"
echo ""
