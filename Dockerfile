FROM node:24-bookworm-slim AS console
WORKDIR /web
RUN corepack enable
COPY web/package.json web/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm run build

FROM rust:1-bookworm AS server
WORKDIR /src
ENV GOAT_MERGE_SKIP_WEB_BUILD=1
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
COPY migrations/ migrations/
COPY --from=console /web/dist/ web/dist/
RUN cargo build --release -p goat-merge

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=server /src/target/release/goat-merge /usr/local/bin/goat-merge
USER 1000:1000
EXPOSE 8080
ENTRYPOINT ["goat-merge"]
CMD ["run"]
