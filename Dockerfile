# syntax=docker/dockerfile:1

# --- 1. Interface web (React, dossier ui/) -----------------------------------
FROM node:22-trixie-slim AS ui
WORKDIR /build/ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY ui ./
RUN npm run build

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
COPY src ./src
RUN cargo build --release --locked

# --- 3. Image d'exécution ----------------------------------------------------
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /build/target/release/bot-discord /usr/local/bin/bot-discord
# Interface web servie par le bot (WEB_UI_DIR, /app/ui par défaut).
COPY --from=ui /build/ui/dist /app/ui

# MALLOC_ARENA_MAX : la glibc crée une arène mémoire par thread, qui gonfle la
# mémoire résidente sans gain pour un bot ; deux suffisent.
ENV TZ=Europe/Paris \
    RUST_LOG=info \
    MALLOC_ARENA_MAX=2
# Site web : interface React, API JSON (/api) et /healthz.
EXPOSE 8890

# Toutes les données (base principale, base et fichiers des tickets) sont sous
# /app/data : un seul volume à monter. Les migrations sont appliquées au démarrage.
# Exécution en root comme l'image précédente, pour reprendre les fichiers des
# volumes existants sans problème de droits.
VOLUME ["/app/data"]
ENTRYPOINT ["bot-discord"]
