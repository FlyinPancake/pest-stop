FROM rust:1.95 AS build

WORKDIR /app

COPY . .

RUN cargo build --release -p pest-stop

# RUN ls -la target/release/pest-stop && exit 1

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/target/release/pest-stop /usr/local/bin/pest-stop

# RUN ls -la /usr/local/bin/pest-stop && exit 1

ENV BIND_ADDR=0.0.0.0

EXPOSE 3000

CMD ["/usr/local/bin/pest-stop"]
