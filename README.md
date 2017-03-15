# Tokio Serialize / Deserialize

Utilities needed to easily implement a Tokio transport using [serde] for
serialization and deserialization of frame values.

[Documentation](https://carllerche.github.io/tokio-serde/tokio_serde/index.html)

## Usage

To use `tokio-serde`, first add this to your `Cargo.toml`:

```toml
[dependencies]
tokio-serde = { git = "https://github.com/carllerche/tokio-serde" }
```

Next, add this to your crate:

```rust
extern crate tokio_serde;

use tokio_serde::{Serializer, Deserializer, FramedRead, FramedWrite};
```

[serde]: https://serde.rs

# License

`tokio-serde` is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
