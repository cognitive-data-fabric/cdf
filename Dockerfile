# Multi-stage Dockerfile for development and production
# Usage:
#   Development: docker build --target dev -t myapp:dev .
#   Production:  docker build --target prod -t myapp:latest .

# --- Base Stage ---
FROM node:20-alpine AS base
WORKDIR /app
RUN apk add --no-cache git

# --- Development Stage ---
FROM base AS dev
ENV NODE_ENV=development
COPY package*.json ./
RUN npm install
COPY . .
EXPOSE 3000
CMD ["npm", "run", "dev"]

# --- Build Stage ---
FROM base AS build
ENV NODE_ENV=production
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

# --- Production Stage ---
FROM nginx:alpine AS prod
COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
