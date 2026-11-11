use core::arch::aarch64::*;

use super::{Error, scalar};

/// # Safety
/// NEON must be available. Output must have exactly twice the input length.
#[target_feature(enable = "neon")]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
    let mut offset = 0;
    // SAFETY: The caller guarantees NEON and disjoint source/destination slices.
    // The alphabet has 16 initialized bytes. Each loop has 16 readable input
    // bytes and, by the checked output length, 32 writable output bytes. All
    // loads/stores support byte alignment; no padding or vector alignment is
    // required. Offsets remain inside their respective allocations.
    unsafe {
        let alphabet = vld1q_u8(if UPPER {
            b"0123456789ABCDEF".as_ptr()
        } else {
            b"0123456789abcdef".as_ptr()
        });
        let mask = vdupq_n_u8(15);
        while input.len() - offset >= 16 {
            let bytes = vld1q_u8(input.as_ptr().add(offset));
            let high = vqtbl1q_u8(alphabet, vshrq_n_u8::<4>(bytes));
            let low = vqtbl1q_u8(alphabet, vandq_u8(bytes, mask));
            vst2q_u8(output.as_mut_ptr().add(offset * 2), uint8x16x2_t(high, low));
            offset += 16;
        }
    }
    scalar::encode::<UPPER>(&input[offset..], &mut output[offset * 2..]);
}

/// # Safety
/// NEON must be available. Input length must be even and output half its length.
#[target_feature(enable = "neon")]
pub(crate) unsafe fn decode(input: &[u8], output: &mut [u8]) -> Result<(), Error> {
    let mut offset = 0;
    // SAFETY: The caller guarantees NEON and disjoint source/destination slices.
    // Full iterations load exactly 32 readable input bytes and store exactly
    // 16 writable output bytes. Byte alignment is sufficient for these NEON
    // loads/stores. offset stays even and bounded; no tail load uses padding.
    // Register-only arithmetic has no additional alignment or alias conditions.
    unsafe {
        let zero = vdupq_n_u8(b'0');
        let a = vdupq_n_u8(b'a');
        let nine = vdupq_n_u8(9);
        let five = vdupq_n_u8(5);
        let ten = vdupq_n_u8(10);
        let case_bit = vdupq_n_u8(0x20);
        let nibble = |chars| {
            let digit = vsubq_u8(chars, zero);
            let letter = vsubq_u8(vorrq_u8(chars, case_bit), a);
            let is_digit = vcleq_u8(digit, nine);
            let valid = vorrq_u8(is_digit, vcleq_u8(letter, five));
            let value = vbslq_u8(is_digit, digit, vaddq_u8(letter, ten));
            (value, valid)
        };
        while input.len() - offset >= 32 {
            let chars = vld2q_u8(input.as_ptr().add(offset));
            let (high, high_valid) = nibble(chars.0);
            let (low, low_valid) = nibble(chars.1);
            if vminvq_u8(vandq_u8(high_valid, low_valid)) != 255 {
                return scalar::decode(&input[offset..], &mut output[offset / 2..], offset);
            }
            let bytes = vorrq_u8(vshlq_n_u8::<4>(high), low);
            vst1q_u8(output.as_mut_ptr().add(offset / 2), bytes);
            offset += 32;
        }
    }
    scalar::decode(&input[offset..], &mut output[offset / 2..], offset)
}
