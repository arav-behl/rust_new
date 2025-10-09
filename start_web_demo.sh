#!/bin/bash

echo "🚀 Starting Wintermute Order Book Web Demo"
echo "=========================================="
echo ""

echo "📋 What this demonstrates:"
echo "   • Real-time order book visualization"
echo "   • Interactive order submission with latency tracking"
echo "   • Live performance metrics dashboard"
echo "   • Sub-microsecond latency measurement"
echo "   • Professional web interface for recruiters"
echo ""

echo "🔧 Building web server..."
if cargo build --release --bin web_server; then
    echo "✅ Build successful!"
else
    echo "❌ Build failed"
    exit 1
fi

echo ""
echo "🌐 Starting web server..."
echo "   Interface will be available at: http://localhost:3000"
echo ""
echo "🎯 Perfect for showing to recruiters:"
echo "   • Click around the interface to submit orders"
echo "   • Watch real-time latency measurements"
echo "   • Observe order book updates"
echo "   • Demonstrate sub-10μs performance"
echo ""
echo "💼 Press Ctrl+C to stop the server"
echo ""

cargo run --release --bin web_server