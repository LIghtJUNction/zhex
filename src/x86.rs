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
        let mut i = 0;
        while input.len() - i >= 32 {
            encode32(
                input.as_ptr().add(i),
                output.as_mut_ptr().add(2 * i),
                table,
                mask,
            );
            i += 32;
        }
        if i != input.len() {
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
        let high = _mm256_shuffle_epi8(table, _mm256_and_si256(_mm256_srli_epi16(bytes, 4), mask));
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

/// Caller guarantees AVX2 + OS support, even input and output.len() == input.len()/2.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn decode(input: &[u8], output: &mut [u8]) -> Result<(), Error> {
    let mut i = 0;
    // SAFETY: CPU/OS AVX2 and exact disjoint lengths are caller invariants.
    // Each full group reads 64 initialized bytes and writes 32 bytes; the
    // remaining loop reads 32 and writes 16. All accesses are unaligned and
    // bounded. Each group's validity is checked before any output store.
    // A final overlapping chunk touches only existing input/output bytes, and
    // its overlapped prefix was already validated. Error indices stay global.
    unsafe {
        while input.len() - i >= 64 {
            let (a, va) = decode32(_mm256_loadu_si256(input.as_ptr().add(i).cast()));
            let (b, vb) = decode32(_mm256_loadu_si256(input.as_ptr().add(i + 32).cast()));
            if _mm256_movemask_epi8(_mm256_or_si256(va, vb)) != 0 {
                let first = _mm256_movemask_epi8(va) as u32;
                let relative = if first != 0 {
                    first.trailing_zeros() as usize
                } else {
                    32 + (_mm256_movemask_epi8(vb) as u32).trailing_zeros() as usize
                };
                let index = i + relative;
                return Err(Error::InvalidByte {
                    index,
                    byte: input[index],
                });
            }
            let bytes = _mm256_permute4x64_epi64(_mm256_packus_epi16(a, b), 0xd8);
            _mm256_storeu_si256(output.as_mut_ptr().add(i / 2).cast(), bytes);
            i += 64;
        }
        if input.len() >= 32 {
            let last = input.len() - 32;
            while i < input.len() {
                // After a full group, a short tail overlaps only validated text.
                i = core::cmp::min(i, last);
                let (pairs, check) = decode32(_mm256_loadu_si256(input.as_ptr().add(i).cast()));
                let invalid = _mm256_movemask_epi8(check) as u32;
                if invalid != 0 {
                    let index = i + invalid.trailing_zeros() as usize;
                    return Err(Error::InvalidByte {
                        index,
                        byte: input[index],
                    });
                }
                let bytes = _mm_packus_epi16(
                    _mm256_castsi256_si128(pairs),
                    _mm256_extracti128_si256(pairs, 1),
                );
                _mm_storeu_si128(output.as_mut_ptr().add(i / 2).cast(), bytes);
                i += 32;
            }
            return Ok(());
        }
        #[cfg(target_feature = "sse2")]
        return crate::sse2::decode(input, output, 0);
    }
    #[cfg(not(target_feature = "sse2"))]
    scalar::decode(input, output, 0)
}

/// Requires AVX2. Returns decoded u16 pairs and sign bits marking invalid bytes.
///
/// The subtract-one hash and two delta tables follow Daniel Lemire's public
/// domain algorithm: https://lemire.me/blog/2023/07/27/decoding-base16-sequences-quickly/
/// Original source: lemire/Code-used-on-Daniel-Lemire-s-blog at
/// 23fa12e7a1f705d3e6fa9589b6d573b078c30618, 2023/07/27/src/base16.c.
#[inline]
#[target_feature(enable = "avx2")]
#[allow(unused_unsafe)] // Register intrinsics require unsafe on the MSRV.
unsafe fn decode32(text: __m256i) -> (__m256i, __m256i) {
    // SAFETY: AVX2 is a caller invariant. All operands are initialized registers;
    // there are no memory, alignment or alias conditions in this helper.
    unsafe {
        let check_delta = _mm256_setr_epi8(
            -16, -32, -47, 71, 58, -96, 26, -128, 0, 0, 0, 0, 0, 0, 0, 0, -16, -32, -47, 71, 58,
            -96, 26, -128, 0, 0, 0, 0, 0, 0, 0, 0,
        );
        let value_delta = _mm256_setr_epi8(
            0, 0, -47, -47, -54, 0, -86, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -47, -47, -54, 0, -86, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
        );
        let shifted = _mm256_add_epi8(text, _mm256_set1_epi8(-1));
        let key = _mm256_and_si256(_mm256_srli_epi16(shifted, 4), _mm256_set1_epi8(15));
        // Subtracting one puts '0' in bucket 2, '1'..'9' in 3, 'A'..'F'
        // in 4, and 'a'..'f' in 6. Only those ranges have a clear check sign.
        let check = _mm256_add_epi8(shifted, _mm256_shuffle_epi8(check_delta, key));
        let nibbles = _mm256_add_epi8(shifted, _mm256_shuffle_epi8(value_delta, key));
        (
            _mm256_maddubs_epi16(nibbles, _mm256_set1_epi16(0x0110)),
            check,
        )
    }
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
