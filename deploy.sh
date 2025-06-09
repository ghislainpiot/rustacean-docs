#!/usr/bin/env bash
set -e

# Simple deployment script for Rustacean Docs MCP Server
# Rebuilds the Docker image and restarts the compose stack

echo "🚀 Starting deployment..."

# Check if docker-compose.yml exists
if [ ! -f "docker-compose.yml" ]; then
    echo "❌ Error: docker-compose.yml not found in current directory"
    exit 1
fi

# Check if Docker and Docker Compose are available
if ! command -v docker &> /dev/null; then
    echo "❌ Error: Docker is not installed or not in PATH"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    echo "❌ Error: Docker Compose is not installed or not in PATH"
    exit 1
fi

# Stop the current stack
echo "🛑 Stopping current deployment..."
docker-compose down

# Remove old images to force rebuild
echo "🧹 Cleaning up old images..."
docker image prune -f
docker-compose down --rmi local 2>/dev/null || true

# Rebuild and start
echo "🔨 Building new image..."
docker-compose build --no-cache

echo "🚀 Starting new deployment..."
docker-compose up -d

# Wait a moment and check if it's running
echo "⏳ Waiting for service to start..."
sleep 5

# Check if container is running
if docker-compose ps | grep -q "Up"; then
    echo "✅ Deployment successful!"
    echo "📊 Service status:"
    docker-compose ps
    echo ""
    echo "📝 Recent logs:"
    docker-compose logs --tail=10
else
    echo "❌ Deployment failed! Container is not running."
    echo "📝 Error logs:"
    docker-compose logs
    exit 1
fi

echo ""
echo "🎉 Deployment complete! Service is running on http://localhost:8080"