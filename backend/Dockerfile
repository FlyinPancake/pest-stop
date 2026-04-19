FROM rust:1.95 AS build

WORKDIR /app

COPY . .

RUN cargo build --release

FROM alpine:latest

WORKDIR /app

COPY --from=build /app/target/release/pest-stop /usr/local/bin/pest-stop

ENV BIND_ADDR=0.0.0.0

EXPOSE 3000

CMD ["pest-stop"]
