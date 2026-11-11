//! A single bounded A/B experiment for bulk encoder address generation.
//!
//! Encoder source copied from f69459e8e58db7d761099a00bb161da34ef638ee.
//! Only its bulk-loop induction changes: increment source/destination pointers
//! instead of indexing both from i. All preflight, lookup and tail work is equal.
use crate::adapters::Codec;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::adapters::preflight;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::{Error, scalar};

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn codecs() -> Vec<Codec> {
    panic!("pointer experiment requires an x86 CPU with AVX2");
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn codecs() -> Vec<Codec> {
    let mut codecs = crate::kernels::codecs()
        .into_iter()
        .filter(|c| c.name == "kernel-avx2-no-short")
        .collect::<Vec<_>>();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if crate::x86::available() {
        codecs.push(Codec {
            name: "candidate-pointer-avx2",
            lower: encode::<false>,
            upper: Some(encode::<true>),
            decode: zhex::decode,
        });
    }
    assert_eq!(codecs.len(), 2, "pointer experiment requires CPU/OS AVX2");
    codecs
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn encode<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    if crate::x86::available() {
        // SAFETY: same preflight, disjoint slices and CPU/OS detection as the
        // production kernel wrapper. All inner accesses are complete chunks.
        unsafe {
            inner::encode::<U>(src, &mut dst[..n]);
        }
    } else {
        scalar::encode::<U>(src, &mut dst[..n]);
    }
    Ok(n)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod inner {
    use crate::scalar;
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    /// Caller guarantees AVX2 + OS support and output.len() == input.len() * 2.
    #[target_feature(enable = "avx2")]
    pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
        if input.len() < 16 {
            return scalar::encode::<UPPER>(input, output);
        }
        // SAFETY: caller verified CPU/OS AVX2 support and exact disjoint lengths.
        // The alphabet contains 32 initialized bytes. Full chunks read 32 and write
        // 64 bytes; the short path reads 16 and writes 32. All accesses are bounded
        // and unaligned. An overlapping last chunk rewrites identical output only.
        unsafe {
            let alphabet = if UPPER {
                b"0123456789ABCDEF0123456789ABCDEF"
            } else {
                b"0123456789abcdef0123456789abcdef"
            };
            let table = _mm256_loadu_si256(alphabet.as_ptr().cast());
            let mask = _mm256_set1_epi8(15);
            if input.len() < 32 {
                let first = encode16(input.as_ptr(), table, mask);
                _mm256_storeu_si256(output.as_mut_ptr().cast(), first);
                if input.len() != 16 {
                    let last = input.len() - 16;
                    let bytes = encode16(input.as_ptr().add(last), table, mask);
                    _mm256_storeu_si256(output.as_mut_ptr().add(2 * last).cast(), bytes);
                }
                return;
            }
            let mut source = input.as_ptr();
            let mut destination = output.as_mut_ptr();
            let end = source.add(input.len() & !31);
            while source != end {
                encode32(source, destination, table, mask);
                source = source.add(32);
                destination = destination.add(64);
            }
            if input.len() & 31 != 0 {
                let last = input.len() - 32;
                encode32(
                    input.as_ptr().add(last),
                    output.as_mut_ptr().add(2 * last),
                    table,
                    mask,
                );
            }
        }
    }

    /// Requires AVX2, 32 readable input and 64 writable output bytes, without overlap.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn encode32(input: *const u8, output: *mut u8, table: __m256i, mask: __m256i) {
        // SAFETY: the caller proves bounded, initialized, disjoint buffers and AVX2.
        // All accesses use loadu/storeu and need only byte alignment.
        unsafe {
            let bytes = _mm256_loadu_si256(input.cast());
            let low = _mm256_shuffle_epi8(table, _mm256_and_si256(bytes, mask));
            let high =
                _mm256_shuffle_epi8(table, _mm256_and_si256(_mm256_srli_epi16(bytes, 4), mask));
            let a = _mm256_unpacklo_epi8(high, low);
            let b = _mm256_unpackhi_epi8(high, low);
            _mm256_storeu_si256(output.cast(), _mm256_permute2x128_si256(a, b, 0x20));
            _mm256_storeu_si256(output.add(32).cast(), _mm256_permute2x128_si256(a, b, 0x31));
        }
    }

    /// AVX2 and 16 readable input bytes are required; table/mask are initialized.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn encode16(input: *const u8, table: __m256i, mask: __m256i) -> __m256i {
        // SAFETY: caller proves AVX2 and 16 initialized readable bytes. loadu needs
        // byte alignment; this helper only reads, with no alias or store condition.
        unsafe {
            let wide = _mm256_cvtepu8_epi16(_mm_loadu_si128(input.cast()));
            let indices = _mm256_and_si256(
                _mm256_or_si256(_mm256_srli_epi16(wide, 4), _mm256_slli_epi16(wide, 8)),
                mask,
            );
            _mm256_shuffle_epi8(table, indices)
        }
    }
}
