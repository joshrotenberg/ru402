# RU402 Rust Examples

These Rust examples roughly follow the Python examples, implemented as a set of
command line example tools.

## Run Redis

Use the supplied `docker-compose.yml` to run a local instance of Redis with Docker. This will also make Redis Insight available on port `8001`:

```shell
docker compose up
```

## Build and Run

This project requires the Rust toolchain to build and run the examples. If you don't already have it, Install it with [rustup](https://rustup.rs/):

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Build and test:

```shell
cargo test
```

### Books

```shell
# by default, the books example will load the data from disk, create the index, and 
# then search for recommendations for two books
cargo run --example books
# the loading can be slow, so to skip it (assuming you've already loaded the data),
# specify false to the load flag
cargo run --example books -- --load false 
# to search for a book instead of the demo ids, specify your own
cargo run --example books -- --load false --book book:112
```

## Resources

* <https://docs.rs/redis/0.25.3/redis/> - the Rust `redis` crate
