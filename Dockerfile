# Builds an image containing the iplookup binary and little else.
FROM rust:slim
COPY . /iplookup
RUN cd /iplookup && cargo build --all-targets --release

FROM busybox
COPY --from=0 /iplookup/target/release/iplookup /iplookup
RUN chmod +x /iplookup