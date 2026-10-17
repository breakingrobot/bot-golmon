# syntax=docker/dockerfile:1

# --- 1. Feuille de style des pages de logs (Tailwind CSS v4) -----------------
FROM node:22-trixie-slim AS css
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/src ./src
# Les classes utilisées sont lues dans les templates et le code du module ticket.
COPY templates /build/templates
COPY src/modmail /build/src/modmail
COPY src/web /build/src/web
RUN npx tailwindcss -i src/app.css -o dist/app.css --minify

# --- 2. Compilation du bot ---------------------------------------------------
FROM rust:1.97-slim-trixie AS build
WORKDIR /build

# Les dépendances sont compilées dans une couche à part : elle reste en cache
# tant que Cargo.toml / Cargo.lock ne changent pas.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src target/release/deps/bot_discord* target/release/bot-discord*

COPY migrations ./migrations
COPY templates ./templates
COPY web/src ./web/src
COPY --from=css /build/web/dist ./web/dist
COPY src ./src
RUN cargo build --release --locked

# --- 3. Image d'exécution ----------------------------------------------------
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /build/target/release/bot-discord /usr/local/bin/bot-discord

ENV TZ=Europe/Paris \
    RUST_LOG=info
# Serveur web des logs de tickets (seulement si le module ticket est configuré).
EXPOSE 8890

# Toutes les données (base principale, base et fichiers des tickets) sont sous
# /app/data : un seul volume à monter. Les migrations sont appliquées au démarrage.
# Exécution en root comme l'image précédente, pour reprendre les fichiers des
# volumes existants sans problème de droits.
VOLUME ["/app/data"]
ENTRYPOINT ["bot-discord"]
