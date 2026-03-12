FROM rust:1.81 AS builder

# This Dockerfile is for compiling the binary and running the BATS test suite.
# Prerequisites: bats submodules must be cloned:
#    git submodule update --init --recursive

WORKDIR /code

COPY . .

RUN cargo build --target-dir /code/targetdocker --release && \
    strip targetdocker/release/rtail && \
    cp targetdocker/release/rtail /usr/bin/

ENTRYPOINT ["/code/test/bats/bin/bats"]
