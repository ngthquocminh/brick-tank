FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p server

FROM debian:bookworm-slim
COPY --from=build /app/target/release/server /usr/local/bin/brick-tank
COPY client /srv/client
ENV STATIC_DIR=/srv/client BIND=0.0.0.0:8080
EXPOSE 8080
CMD ["brick-tank"]
