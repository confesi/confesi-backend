FROM node:18-slim
WORKDIR /app
RUN mkdir node_modules && chown node:node node_modules
USER node
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
CMD ["node", "."]
