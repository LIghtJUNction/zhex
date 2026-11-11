//! Baseline x86 SIMD for small buffers and bounded AVX2 tails.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::{Error, scalar};

/// Caller guarantees SSE2 + OS support, even input and exact half-sized output.
/// `offset` is the start of this slice in the original input.
#[inline]
#[target_feature(enable = "sse2")]
pub(crate) unsafe fn decode(input: &[u8], output: &mut [u8], offset: usize) -> Result<(), Error> {
    if input.len() < 16 {
        return scalar::decode(input, output, offset);
    }
    let last = input.len() - 16;
    let mut i = 0;
    // SAFETY: the compilation target guarantees CPU/OS SSE2 support. Each loop
    // reads exactly 16 in-bounds text bytes, then writes 8 in-bounds result bytes.
    // These unaligned accesses need only byte alignment and the slices cannot
    // alias. Only the low 16 movemask bits describe loaded bytes; invalid input
    // is rejected before a store. The final vector may overlap a prior vector:
    // its start is even, its end is exactly input.len(), and any overlapping
    // bytes were already validated. Thus it preserves the first error position
    // and rewrites only complete, in-bounds output pairs without padding.
    unsafe {
        loop {
            let text = _mm_loadu_si128(input.as_ptr().add(i).cast());
            let digits = _mm_add_epi8(text, _mm_set1_epi8(80));
            let letters = _mm_add_epi8(_mm_or_si128(text, _mm_set1_epi8(32)), _mm_set1_epi8(31));
            let is_digit = _mm_cmpgt_epi8(_mm_set1_epi8(-118), digits);
            let is_letter = _mm_cmpgt_epi8(_mm_set1_epi8(-122), letters);
            let invalid = !(_mm_movemask_epi8(_mm_or_si128(is_digit, is_letter)) as u16);
            if invalid != 0 {
                let index = i + invalid.trailing_zeros() as usize;
                return Err(Error::InvalidByte {
                    index: offset + index,
                    byte: input[index],
                });
            }
            let nibbles = _mm_add_epi8(
                _mm_and_si128(text, _mm_set1_epi8(15)),
                _mm_and_si128(is_letter, _mm_set1_epi8(9)),
            );
            let pairs = _mm_or_si128(
                _mm_and_si128(_mm_slli_epi16(nibbles, 4), _mm_set1_epi16(255)),
                _mm_srli_epi16(nibbles, 8),
            );
            let bytes = _mm_packus_epi16(pairs, _mm_setzero_si128());
            _mm_storel_epi64(output.as_mut_ptr().add(i / 2).cast(), bytes);
            if i == last {
                break;
            }
            i = (i + 16).min(last);
        }
    }
    Ok(())
}
