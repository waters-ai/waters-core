# Dockerfile — Multi-stage build for React/Vite SPA
# Финальный образ: nginx:alpine со статикой

# === Stage 1: Build ===
FROM node:20-alpine AS build

WORKDIR /app

COPY package*.json ./
RUN npm ci --only=production

COPY . .
RUN npm run build

# === Stage 2: Serve ===
FROM nginx:alpine

# Копируем собранную статику
COPY --from=build /app/dist /usr/share/nginx/html

# Nginx конфиг для SPA
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:80/ || exit 1

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
