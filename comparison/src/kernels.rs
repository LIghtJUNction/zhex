//! Checked wrappers around the exact production kernels, with no short cutoff.
use crate::adapters::{Codec, preflight};
use crate::{Error, scalar};

pub fn codecs() -> Vec<Codec> {
    let mut codecs = vec![Codec {
        name: "kernel-scalar",
        lower: scalar_encode::<false>,
        upper: Some(scalar_encode::<true>),
        decode: scalar_decode,
    }];
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if crate::x86::available() {
        codecs.push(Codec {
            name: "kernel-avx2-no-short",
            lower: x86_encode::<false>,
            upper: Some(x86_encode::<true>),
            decode: x86_decode,
        });
    } else {
        eprintln!(
            "kernel-avx2-no-short unavailable: CPU/OS lacks AVX2; production falls back to scalar"
        );
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    codecs.push(Codec {
        name: "kernel-neon-no-short",
        lower: neon_encode::<false>,
        upper: Some(neon_encode::<true>),
        decode: neon_decode,
    });
    codecs
}

fn scalar_encode<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    scalar::encode::<U>(src, &mut dst[..n]);
    Ok(n)
}

fn scalar_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    scalar::decode(src, &mut dst[..n], 0)?;
    Ok(n)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn x86_encode<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    if crate::x86::available() {
        // SAFETY: exact disjoint slice lengths from preflight; available() checks
        // CPU+OS AVX2 support. Kernels handle unaligned complete chunks and tails.
        unsafe {
            crate::x86::encode::<U>(src, &mut dst[..n]);
        }
    } else {
        scalar::encode::<U>(src, &mut dst[..n]);
    }
    Ok(n)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn x86_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    if crate::x86::available() {
        // SAFETY: even input and exact disjoint output from preflight, with
        // CPU+OS AVX2 support verified; no padding or extra alignment needed.
        unsafe {
            crate::x86::decode(src, &mut dst[..n])?;
        }
    } else {
        scalar::decode(src, &mut dst[..n], 0)?;
    }
    Ok(n)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn neon_encode<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    // SAFETY: this compilation target guarantees NEON. Exact disjoint slices
    // from preflight; kernel uses unaligned complete chunks and bounded tails.
    unsafe {
        crate::neon::encode::<U>(src, &mut dst[..n]);
    }
    Ok(n)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn neon_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    // SAFETY: compile-target NEON, even input and exact disjoint output. Loads
    // and stores require no extra alignment and touch only complete chunks.
    unsafe {
        crate::neon::decode(src, &mut dst[..n])?;
    }
    Ok(n)
}
