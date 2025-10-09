#!/bin/bash

echo "🚀 Shuttle.rs Deployment Script"
echo "================================"
echo ""

# Check if logged in
if ! cargo shuttle project list &>/dev/null; then
    echo "❌ Not logged in to Shuttle"
    echo ""
    echo "Please run these commands:"
    echo "  1. cargo shuttle login"
    echo "  2. Follow the browser login"
    echo "  3. Run this script again"
    exit 1
fi

echo "✅ Logged in to Shuttle"
echo ""
echo "🔨 Deploying to Shuttle..."
echo ""

cargo shuttle deploy --allow-dirty

echo ""
echo "✅ Deployment complete!"
echo "Your app should be live at: https://trading-engine.shuttleapp.rs"
