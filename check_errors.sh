#!/bin/bash

echo "🔍 Checking Compilation Errors..."
echo "================================="
echo ""

# Run cargo check and capture detailed output
echo "Running cargo check with verbose output..."
cargo check --verbose 2>&1 | tee compilation_errors.log

echo ""
echo "📋 Error Summary:"
echo "=================="

if grep -q "error\[E" compilation_errors.log; then
    echo "❌ Found compilation errors:"
    grep "error\[E" compilation_errors.log | head -10
elif grep -q "error:" compilation_errors.log; then
    echo "❌ Found errors:"
    grep "error:" compilation_errors.log | head -10
else
    echo "✅ No obvious errors found in output"
fi

echo ""
echo "Full log saved to: compilation_errors.log"
echo "Run 'cat compilation_errors.log' to see all details"