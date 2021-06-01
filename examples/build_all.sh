#!/bin/sh
# Build all the example pdf documents.
# As we add additional optional features, we will want to try all
# combinations and how they interact with one another.
cargo run --example demo demo.pdf \
   && cargo run --features images --example demo demo-with-images.pdf \
   && cargo run --features images --example images images.pdf
