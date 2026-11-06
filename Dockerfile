FROM rust:1.99.1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations
COPY web ./web
RUN cargo build --release --locked -p nexus-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home nexus
COPY --from=build /app/target/release/nexus-api /usr/local/bin/nexus-api
USER 10001
ENV NEXUS_BIND=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s CMD curl -fsS http://127.0.0.1:8080/readyz || exit 1
ENTRYPOINT ["nexus-api"]
