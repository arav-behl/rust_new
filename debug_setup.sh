#!/bin/bash

echo "🔧 Debugging Wintermute Order Book Engine Setup"
echo "=============================================="
echo ""

# Check Rust installation
echo "1️⃣ Checking Rust Installation..."
if command -v cargo &> /dev/null; then
    echo "✅ Cargo found: $(cargo --version)"
    echo "✅ Rustc found: $(rustc --version)"
else
    echo "❌ Rust not installed. Install with:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "   source ~/.cargo/env"
    exit 1
fi
echo ""

# Check project files
echo "2️⃣ Checking Project Structure..."
if [ -f "Cargo.toml" ]; then
    echo "✅ Cargo.toml found"
else
    echo "❌ Cargo.toml not found - are you in the project directory?"
    exit 1
fi

if [ -d "src" ]; then
    echo "✅ src/ directory found"
    echo "   Files in src/: $(ls src/ | wc -l)"
else
    echo "❌ src/ directory not found"
    exit 1
fi
echo ""

# Try to compile and identify specific issues
echo "3️⃣ Testing Compilation..."
echo "Running: cargo check"
echo ""

if cargo check 2>&1 | tee /tmp/cargo_check.log; then
    echo "✅ Compilation check passed"
else
    echo "❌ Compilation issues found. Common fixes:"
    echo ""

    # Check for common issues
    if grep -q "cannot find" /tmp/cargo_check.log; then
        echo "🔍 Missing dependencies or imports detected"
        echo "   Try: cargo update"
    fi

    if grep -q "trait bounds" /tmp/cargo_check.log; then
        echo "🔍 Trait bound issues detected"
        echo "   Some async traits may need boxing"
    fi

    if grep -q "unused" /tmp/cargo_check.log; then
        echo "🔍 Unused imports detected (warnings only)"
        echo "   These won't prevent compilation"
    fi

    echo ""
    echo "Detailed error log:"
    cat /tmp/cargo_check.log
fi
echo ""

# Try a simple test
echo "4️⃣ Testing Simple Build..."
if cargo build --bin engine 2>/dev/null; then
    echo "✅ Binary build successful"
else
    echo "❌ Binary build failed - trying library only..."
    if cargo build --lib 2>/dev/null; then
        echo "✅ Library build successful"
        echo "💡 Issue likely in main.rs or binary dependencies"
    else
        echo "❌ Library build also failed"
        echo "💡 Core dependency or syntax issues present"
    fi
fi
echo ""

# Check dependencies
echo "5️⃣ Checking Dependencies..."
echo "Tokio version: $(cargo tree | grep tokio | head -1 || echo 'Not found')"
echo "Serde version: $(cargo tree | grep serde | head -1 || echo 'Not found')"
echo ""

# System info
echo "6️⃣ System Information..."
echo "OS: $(uname -s)"
echo "Architecture: $(uname -m)"
if command -v sysctl &> /dev/null; then
    echo "CPU Cores: $(sysctl -n hw.ncpu)"
else
    echo "CPU Cores: $(nproc 2>/dev/null || echo 'Unknown')"
fi
echo ""

echo "🎯 Recommended Next Steps:"
echo "1. If compilation failed, fix the errors shown above"
echo "2. Run: cargo update"
echo "3. Try: cargo build --release"
echo "4. Then run: ./run_demo_fixed.sh"
echo ""

cleanup() {
    rm -f /tmp/cargo_check.log
}
trap cleanup EXIT