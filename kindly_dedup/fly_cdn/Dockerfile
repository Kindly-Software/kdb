FROM nginx:alpine

# Copy static HTML
COPY index.html /usr/share/nginx/html/index.html
COPY nginx.conf /etc/nginx/nginx.conf

# Health check endpoint
RUN echo "OK" > /usr/share/nginx/html/health

# Set permissions
RUN chown -R nginx:nginx /usr/share/nginx/html

EXPOSE 8080

CMD ["nginx", "-g", "daemon off;"]
