//! Hexadecimal conversion using only `core` and caller-owned buffers.
//!
//! ```
//! let mut text = [0; 8];
//! assert_eq!(zhex::encode_lower(&[0, 18, 171, 255], &mut text), Ok(8));
//! assert_eq!(&text, b"0012abff");
//! let mut bytes = [0; 4];
//! assert_eq!(zhex::decode(b"0012aBfF", &mut bytes), Ok(4));
//! assert_eq!(bytes, [0, 18, 171, 255]);
//! ```
//!
//! Length and capacity errors leave the entire output unchanged. Decoding
//! validates while converting: an invalid byte may leave earlier output changed.
//! The amount written on failure is unspecified; the error always identifies the
//! first invalid byte in the original input. Bytes beyond a successful result's
//! returned length are unchanged. No input or output padding is required.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

use core::fmt;

mod scalar;

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    not(target_env = "sgx"),
    not(feature = "force-scalar"),
    not(miri)
))]
mod x86;

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_env = "sgx"),
    not(feature = "force-scalar"),
    not(miri)
))]
mod sse2;

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    not(feature = "force-scalar"),
    not(miri)
))]
mod neon;

/// A conversion error, in preflight-then-content precedence order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Doubling the input length would overflow `usize`.
    LengthOverflow,
    /// Hexadecimal text has an odd number of bytes.
    OddLength,
    /// The caller's output buffer is too short.
    OutputTooSmall {
        /// Required number of output bytes.
        required: usize,
        /// Number of output bytes provided.
        available: usize,
    },
    /// The first byte outside ASCII `0..=9`, `a..=f`, and `A..=F`.
    InvalidByte {
        /// Zero-based byte position in the original input.
        index: usize,
        /// The offending byte (not a Unicode character).
        byte: u8,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::LengthOverflow => f.write_str("hexadecimal output length overflows usize"),
            Self::OddLength => f.write_str("hexadecimal input length is odd"),
            Self::OutputTooSmall {
                required,
                available,
            } => {
                write!(f, "output needs {required} bytes, but has {available}")
            }
            Self::InvalidByte { index, byte } => {
                write!(f, "invalid hexadecimal byte 0x{byte:02x} at index {index}")
            }
        }
    }
}

/// Calculate an encoding's output length, rejecting overflow.
#[inline]
pub const fn encoded_len(input_len: usize) -> Result<usize, Error> {
    match input_len.checked_mul(2) {
        Some(len) => Ok(len),
        None => Err(Error::LengthOverflow),
    }
}

/// Calculate a decoding's output length, rejecting odd input lengths.
///
/// This checks the length only; [`decode`] validates the input bytes.
#[inline]
pub const fn decoded_len(input_len: usize) -> Result<usize, Error> {
    if input_len % 2 == 0 {
        Ok(input_len / 2)
    } else {
        Err(Error::OddLength)
    }
}

/// Encode bytes as lowercase ASCII hexadecimal, returning the bytes written.
///
/// Checks length overflow, then output capacity, before writing anything.
#[inline]
pub fn encode_lower(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    encode::<false>(input, output)
}

/// Encode bytes as uppercase ASCII hexadecimal, returning the bytes written.
///
/// Checks length overflow, then output capacity, before writing anything.
#[inline]
pub fn encode_upper(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    encode::<true>(input, output)
}

#[inline]
fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    let len = encoded_len(input.len())?;
    check_capacity(len, output.len())?;
    let output = &mut output[..len];
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        not(target_env = "sgx"),
        not(feature = "force-scalar"),
        not(miri)
    ))]
    if input.len() >= 16 && x86::available() {
        // SAFETY: available() verifies AVX2 and OS vector-state support. The
        // slices are non-aliasing and preflight sized the output exactly. The
        // backend uses unaligned accesses within complete, bounded chunks.
        unsafe { x86::encode::<UPPER>(input, output) };
        return Ok(len);
    }
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        not(feature = "force-scalar"),
        not(miri)
    ))]
    if input.len() >= 16 {
        // SAFETY: NEON is required by this compilation target. Preflight sized
        // output exactly; distinct slices cannot alias, and the backend handles
        // unaligned chunks and the bounded tail without padding.
        unsafe { neon::encode::<UPPER>(input, output) };
        return Ok(len);
    }
    scalar::encode::<UPPER>(input, output);
    Ok(len)
}

/// Decode mixed-case ASCII hexadecimal, returning the bytes written.
///
/// Rejects odd length first, insufficient capacity second, then the first invalid
/// input byte. Whitespace, `0x` prefixes and non-ASCII bytes are not accepted.
/// Length/capacity errors preserve the output. On invalid input, an unspecified
/// prefix of the output may already have changed; no full prescan is performed.
#[inline]
pub fn decode(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    let len = decoded_len(input.len())?;
    check_capacity(len, output.len())?;
    let output = &mut output[..len];
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        not(target_env = "sgx"),
        not(feature = "force-scalar"),
        not(miri)
    ))]
    if input.len() >= 32 && x86::available() {
        // SAFETY: available() verifies CPU and OS AVX2 support; the disjoint
        // output has exactly half the even input length. Backend loads/stores
        // need only byte alignment and stay within complete chunks.
        unsafe { x86::decode(input, output) }?;
        return Ok(len);
    }
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse2",
        not(target_env = "sgx"),
        not(feature = "force-scalar"),
        not(miri)
    ))]
    if input.len() >= 16 {
        // SAFETY: this target guarantees CPU/OS SSE2 support. Even input and
        // exact output length were checked; the slices cannot alias, and all
        // backend loads/stores are bounded and support byte alignment.
        unsafe { sse2::decode(input, output, 0) }?;
        return Ok(len);
    }
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        not(feature = "force-scalar"),
        not(miri)
    ))]
    if input.len() >= 32 {
        // SAFETY: this target guarantees NEON. Even input and exact output
        // capacity were checked. Slices cannot alias; unaligned accesses remain
        // within complete chunks and the scalar tail.
        unsafe { neon::decode(input, output) }?;
        return Ok(len);
    }
    scalar::decode(input, output, 0)?;
    Ok(len)
}

#[inline]
fn check_capacity(required: usize, available: usize) -> Result<(), Error> {
    if available < required {
        Err(Error::OutputTooSmall {
            required,
            available,
        })
    } else {
        Ok(())
    }
}
