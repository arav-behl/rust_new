#!/bin/bash

echo "🚀 Starting Wintermute Order Book Web Demo"
echo "=========================================="
echo ""

echo "📋 Professional Web Interface Features:"
echo "   • Real-time order submission and processing"
echo "   • Live latency measurements (sub-10μs tracking)"
echo "   • Interactive order book visualization"
echo "   • Performance metrics dashboard"
echo "   • Professional UI perfect for recruiters"
echo ""

echo "🔧 Installing Python dependencies..."
if ! command -v python3 &> /dev/null; then
    echo "❌ Python3 not found. Please install Python3 first."
    exit 1
fi

# Install dependencies
pip3 install -r requirements.txt

echo ""
echo "🌐 Starting Streamlit web interface..."
echo "   Interface will be available at: http://localhost:8501"
echo ""
echo "🎯 Perfect for showing to recruiters:"
echo "   • Click 'Submit Order' to see real-time latency tracking"
echo "   • Watch the latency chart update in real-time"
echo "   • Observe order book updates and spread calculations"
echo "   • Demonstrate professional trading interface"
echo ""
echo "💼 Press Ctrl+C to stop the server"
echo ""

streamlit run web_demo.py --server.port 8501 --server.address localhost