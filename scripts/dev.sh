#!/bin/bash
# Development helper script for Linux/macOS

set -e

ACTION=${1:-start}

case $ACTION in
    start)
        echo -e "\033[0;32mStarting development environment...\033[0m"
        docker-compose up -d
        echo -e "\033[0;36mApp running at http://localhost:3000\033[0m"
        ;;
    stop)
        echo -e "\033[0;33mStopping development environment...\033[0m"
        docker-compose down
        ;;
    build)
        echo -e "\033[0;32mBuilding production image...\033[0m"
        docker build --target prod -t myapp:latest .
        ;;
    test)
        echo -e "\033[0;32mRunning tests...\033[0m"
        docker-compose exec app npm test
        ;;
    lint)
        echo -e "\033[0;32mRunning linter...\033[0m"
        docker-compose exec app npm run lint
        ;;
    clean)
        echo -e "\033[0;31mCleaning up containers and volumes...\033[0m"
        docker-compose down -v --remove-orphans
        docker system prune -f
        ;;
    *)
        echo "Usage: $0 {start|stop|build|test|lint|clean}"
        exit 1
        ;;
esac
