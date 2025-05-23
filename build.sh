#!/bin/bash

# Solana Degen Scanner Build Script
# This script helps set up the scanner with all its dependencies

set -e  # Exit on any error

echo "🔍 Solana Degen Scanner Setup"
echo "================================"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust not found. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust found: $(rustc --version)"

# Copy environment template if .env doesn't exist
if [ ! -f .env ]; then
    if [ -f env.example ]; then
        cp env.example .env
        echo "✅ Created .env from template"
        echo "   Edit .env to configure your endpoints"
    else
        echo "⚠️  No env.example found, creating basic .env"
        cat > .env << EOF
# Solana RPC endpoint
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# Yellowstone gRPC endpoint (optional)
YELLOWSTONE_GRPC_URL=http://grpc.solana.com:10000

# Enable gRPC streaming
USE_GRPC=true

# Logging level
RUST_LOG=info
EOF
    fi
else
    echo "✅ .env file already exists"
fi

# Build the project
echo ""
echo "🔨 Building project..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
else
    echo "❌ Build failed!"
    exit 1
fi

echo ""
echo "🚀 Setup complete!"
echo ""
echo "Next steps:"
echo "1. Edit .env to configure your endpoints"
echo "2. Run the scanner:"
echo "   cargo run --release"
echo ""
echo "Or use these quick commands:"
echo "   # Real-time streaming mode (recommended)"
echo "   USE_GRPC=true cargo run --release"
echo ""
echo "   # Legacy RPC polling mode"
echo "   USE_GRPC=false cargo run --release"
echo ""
echo "   # Debug mode"
echo "   RUST_LOG=debug cargo run --release"

# Make the built binary easily accessible
if [ -f target/release/solana-scanner ]; then
    echo ""
    echo "📦 Binary available at: target/release/solana-scanner"
    echo "   You can also run: ./target/release/solana-scanner"
fi 