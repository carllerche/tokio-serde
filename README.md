# Tokio Serialize / Deserialize

Utilities needed to easily implement a Tokio transport using [serde] for
serialization and deserialization of frame values.

[Documentation](https://carllerche.github.io/tokio-serde/tokio_serde/index.html)

## Usage

To use `tokio-serde`, first add this to your `Cargo.toml`:

```toml
[dependencies]
tokio-serde = "0.3"
```

Next, add this to your crate:

```rust
use tokio_serde::{Serializer, Deserializer, FramedRead, FramedWrite};
```

[serde]: https://serde.rs

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in iovec by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
