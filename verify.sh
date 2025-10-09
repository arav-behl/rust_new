#!/bin/bash
# Quick verification script to ensure everything works

echo "🔍 Verifying Rust Crypto Trading Project..."
echo ""

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust."
    exit 1
fi

echo "✅ Rust/Cargo found"
echo ""

# Build the project
echo "📦 Building project..."
if cargo build --bin simple_trading_engine --release 2>&1 | grep -q "Finished"; then
    echo "✅ Build successful"
else
    echo "❌ Build failed"
    exit 1
fi
echo ""

# Run tests
echo "🧪 Running tests..."
if cargo test --test integration_test 2>&1 | grep -q "test result: ok"; then
    echo "✅ All tests passed"
else
    echo "❌ Tests failed"
    exit 1
fi
echo ""

# Check if binary exists
if [ -f "target/release/simple_trading_engine" ]; then
    echo "✅ Binary created successfully"
else
    echo "❌ Binary not found"
    exit 1
fi
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Project is ready for resume!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "To run the project:"
echo "  cargo run --bin simple_trading_engine --release"
echo ""
echo "To test:"
echo "  cargo test --test integration_test"
echo ""
echo "Next steps:"
echo "1. Read README.md for project overview"
echo "2. Read RECRUITER_PITCH.md for interview prep"
echo "3. Read PROJECT_STATUS.md for full status"
echo ""
