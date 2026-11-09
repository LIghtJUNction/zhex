//! AVX2 kernels; all memory operations use complete, unaligned chunks.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::{scalar, Error};

#[inline]
pub(crate) fn available() -> bool {
    #[cfg(target_feature = "avx2")]
    {
        // A binary compiled for AVX2 already requires CPU and OS AVX2 support.
        true
    }
    #[cfg(not(target_feature = "avx2"))]
    {
        use core::sync::atomic::{AtomicU8, Ordering};
        static DETECTED: AtomicU8 = AtomicU8::new(0);
        let cached = DETECTED.load(Ordering::Relaxed);
        if cached != 0 {
            return cached == 2;
        }
        let supported = detect();
        // This caches a value, not other memory; concurrent duplicate detection
        // is harmless. No acquire/release synchronization is needed.
        DETECTED.store(if supported { 2 } else { 1 }, Ordering::Relaxed);
        supported
    }
}

#[cfg(not(target_feature = "avx2"))]
fn detect() -> bool {
    // SAFETY: supported x86 targets have CPUID; SGX is excluded by cfg.
    // detect_with calls XGETBV only after CPUID confirms XSAVE, OSXSAVE and AVX.
    // XCR0 exists; these register-only operations have no memory, alignment or
    // alias requirements. CPUID itself also requires unsafe on the MSRV.
    unsafe { detect_with(|leaf| __cpuid_count(leaf, 0), || _xgetbv(0)) }
}

#[cfg(not(target_feature = "avx2"))]
fn detect_with(cpuid: impl Fn(u32) -> CpuidResult, xcr0: impl Fn() -> u64) -> bool {
    if cpuid(0).eax < 7 {
        return false;
    }
    let required = (1 << 26) | (1 << 27) | (1 << 28);
    // Short-circuiting is essential: XGETBV needs all three prerequisite bits.
    // XCR0 bits 1 and 2 require OS support for both XMM and YMM state.
    if cpuid(1).ecx & required != required || xcr0() & 6 != 6 {
        return false;
    }
    cpuid(7).ebx & (1 << 5) != 0
}

/// Caller guarantees AVX2 + OS support and output.len() == input.len() * 2.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
    let mut i = 0;
    // SAFETY: caller verified CPU/OS AVX2 support and exact output capacity.
    // The loop's remaining-length check bounds every 32-byte load and two
    // 32-byte stores. loadu/storeu need only byte alignment. Input is shared and
    // output uniquely borrowed, so the buffers do not alias. The alphabet has
    // 32 initialized bytes; the scalar tail accesses only unprocessed slices.
    unsafe {
        let alphabet = if UPPER {
            b"0123456789ABCDEF0123456789ABCDEF"
        } else {
            b"0123456789abcdef0123456789abcdef"
        };
        let table = _mm256_loadu_si256(alphabet.as_ptr().cast());
        let mask = _mm256_set1_epi8(15);
        while input.len() - i >= 32 {
            let bytes = _mm256_loadu_si256(input.as_ptr().add(i).cast());
            let low = _mm256_shuffle_epi8(table, _mm256_and_si256(bytes, mask));
            let high =
                _mm256_shuffle_epi8(table, _mm256_and_si256(_mm256_srli_epi16(bytes, 4), mask));
            let a = _mm256_unpacklo_epi8(high, low);
            let b = _mm256_unpackhi_epi8(high, low);
            _mm256_storeu_si256(
                output.as_mut_ptr().add(2 * i).cast(),
                _mm256_permute2x128_si256(a, b, 0x20),
            );
            _mm256_storeu_si256(
                output.as_mut_ptr().add(2 * i + 32).cast(),
                _mm256_permute2x128_si256(a, b, 0x31),
            );
            i += 32;
        }
    }
    scalar::encode::<UPPER>(&input[i..], &mut output[2 * i..]);
}

/// Caller guarantees AVX2 + OS support, even input and output.len() == input.len()/2.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn decode(input: &[u8], output: &mut [u8]) -> Result<(), Error> {
    let mut i = 0;
    // SAFETY: caller verified CPU/OS support and exact disjoint slice lengths.
    // Every iteration reads 32 in-bounds input bytes and writes 16 in-bounds
    // output bytes, using unaligned accesses. The error mask is checked before
    // storing, and trailing_zeros yields a position within the current chunk.
    unsafe {
        while input.len() - i >= 32 {
            let text = _mm256_loadu_si256(input.as_ptr().add(i).cast());
            // Bias each accepted unsigned interval to the bottom of the signed
            // byte range: digits become -128..=-119, letters -128..=-123.
            // One signed comparison then validates each entire interval.
            let digits = _mm256_add_epi8(text, _mm256_set1_epi8(80));
            let letters = _mm256_add_epi8(
                _mm256_or_si256(text, _mm256_set1_epi8(32)),
                _mm256_set1_epi8(31),
            );
            let is_digit = _mm256_cmpgt_epi8(_mm256_set1_epi8(-118), digits);
            let is_letter = _mm256_cmpgt_epi8(_mm256_set1_epi8(-122), letters);
            let invalid = !(_mm256_movemask_epi8(_mm256_or_si256(is_digit, is_letter)) as u32);
            if invalid != 0 {
                let index = i + invalid.trailing_zeros() as usize;
                return Err(Error::InvalidByte {
                    index,
                    byte: input[index],
                });
            }
            let nibbles = _mm256_add_epi8(
                _mm256_and_si256(text, _mm256_set1_epi8(15)),
                _mm256_and_si256(is_letter, _mm256_set1_epi8(9)),
            );
            let pairs = _mm256_maddubs_epi16(nibbles, _mm256_set1_epi16(0x0110));
            let bytes = _mm_packus_epi16(
                _mm256_castsi256_si128(pairs),
                _mm256_extracti128_si256(pairs, 1),
            );
            _mm_storeu_si128(output.as_mut_ptr().add(i / 2).cast(), bytes);
            i += 32;
        }
    }
    scalar::decode(&input[i..], &mut output[i / 2..], i)
}

#[cfg(test)]
mod tests {
    extern crate std;

    #[test]
    fn detector_agrees_with_standard_library() {
        assert_eq!(super::available(), std::is_x86_feature_detected!("avx2"));
    }

    #[cfg(not(target_feature = "avx2"))]
    #[test]
    fn detection_rejects_missing_cpu_or_os_support_in_order() {
        const REQUIRED: u32 = (1 << 26) | (1 << 27) | (1 << 28);
        fn probe(max_leaf: u32, ecx: u32, xcr0: u64, ebx: u32) -> bool {
            let step = core::cell::Cell::new(0);
            super::detect_with(
                |leaf| {
                    let mut result = super::CpuidResult {
                        eax: 0,
                        ebx: 0,
                        ecx: 0,
                        edx: 0,
                    };
                    match leaf {
                        0 => {
                            assert_eq!(step.replace(1), 0);
                            result.eax = max_leaf;
                        }
                        1 => {
                            assert!(max_leaf >= 7);
                            assert_eq!(step.replace(2), 1);
                            result.ecx = ecx;
                        }
                        7 => {
                            assert_eq!(xcr0 & 6, 6, "leaf 7 before OS support");
                            assert_eq!(step.replace(4), 3);
                            result.ebx = ebx;
                        }
                        _ => panic!("unexpected CPUID leaf"),
                    }
                    result
                },
                || {
                    assert_eq!(ecx & REQUIRED, REQUIRED, "XGETBV before prerequisites");
                    assert_eq!(step.replace(3), 2);
                    xcr0
                },
            )
        }
        for max_leaf in [0, 1, 6] {
            assert!(!probe(max_leaf, REQUIRED, 6, 32));
        }
        for bit in [26, 27, 28] {
            assert!(!probe(7, REQUIRED & !(1 << bit), 6, 32));
        }
        for xcr0 in [0, 2, 4] {
            assert!(!probe(7, REQUIRED, xcr0, 32));
        }
        assert!(!probe(7, REQUIRED, 6, 0));
        assert!(probe(7, REQUIRED, 6, 32));
        assert!(probe(u32::MAX, u32::MAX, u64::MAX, u32::MAX));
    }
}
