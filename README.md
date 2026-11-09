# zhex

Hexadecimal encoding and decoding into caller-owned buffers. The crate always
uses `#![no_std]` and only `core`: no allocation, dependencies, build script, or
external runtime. Stable Rust 1.70 or newer.

```rust
let mut text = [0; 8];
assert_eq!(zhex::encode_lower(&[0, 18, 171, 255], &mut text), Ok(8));
assert_eq!(&text, b"0012abff");

let mut bytes = [0; 4];
assert_eq!(zhex::decode(b"0012aBfF", &mut bytes), Ok(4));
assert_eq!(bytes, [0, 18, 171, 255]);
```

`encode_upper` produces uppercase text. `encoded_len` checks multiplication
overflow; `decoded_len` rejects odd lengths. All functions return `Result`.
An oversized output is accepted and its unused suffix stays unchanged.

Decoding accepts only ASCII `0–9`, `a–f`, and `A–F`. Empty input succeeds.
Error precedence is odd length, insufficient capacity, then the first invalid
byte. `Error::InvalidByte { index, byte }` uses the original input byte offset.
Encoding checks overflow before capacity. Length and capacity errors leave all
output unchanged. An invalid byte may leave an unspecified output prefix
modified; decoding checks characters as it goes, without a full prescan.

The standalone `comparison/` project is excluded from the crate's workspace and
published package. Its third-party dependencies never enter zhex's dependency
tree. See the benchmark protocol there for reproducible comparisons.

MIT licensed.
