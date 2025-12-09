FROM nginx:alpine

# Copy custom nginx config
COPY nginx.conf /etc/nginx/nginx.conf

# Create directory for binaries
RUN mkdir -p /usr/share/nginx/html/releases/latest && \
    mkdir -p /usr/share/nginx/html/releases/v2.0.0

# Set permissions
RUN chown -R nginx:nginx /usr/share/nginx/html

EXPOSE 8080

CMD ["nginx", "-g", "daemon off;"]
