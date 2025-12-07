# Simple nginx container serving pre-built WASM
FROM nginx:alpine

# Copy pre-built dist folder
COPY dist /usr/share/nginx/html

# Custom nginx config for SPA routing
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 8080

CMD ["nginx", "-g", "daemon off;"]
