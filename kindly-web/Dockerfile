FROM nginx:alpine

# Copy WASM bundle to nginx html directory
COPY dist/ /usr/share/nginx/html/

# Create nginx configuration
RUN cat > /etc/nginx/nginx.conf <<'EOF'
user nginx;
worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    # Add WASM MIME type
    types {
        application/wasm wasm;
    }

    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for"';

    access_log /var/log/nginx/access.log main;

    sendfile on;
    tcp_nopush on;
    keepalive_timeout 65;

    # Enable gzip compression
    gzip on;
    gzip_types application/javascript application/wasm text/css text/html;
    gzip_min_length 256;

    server {
        listen 8080;
        server_name _;
        root /usr/share/nginx/html;
        index index.html;

        # Cache static assets
        location ~* \.(wasm|js)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }

        # Serve index.html for all routes (SPA)
        location / {
            try_files $uri $uri/ /index.html;
        }
    }
}
EOF

# Expose port 8080
EXPOSE 8080

# Start nginx
CMD ["nginx", "-g", "daemon off;"]
