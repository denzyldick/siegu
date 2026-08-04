FROM node:20-alpine

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci --omit=dev

COPY . .

# Create data directory for CSV storage
RUN mkdir -p data

EXPOSE 80

CMD ["node", "server.js"]
