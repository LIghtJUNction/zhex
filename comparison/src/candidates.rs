//! Safe prototypes for measuring scalar choices; none is part of the library.
// Keep the same slice APIs as the MSRV-compatible production candidates.
#![allow(clippy::chunks_exact_to_as_chunks)]
use crate::adapters::{preflight, Codec};
use zhex::Error;

pub const CODECS: &[Codec] = &[
    Codec {
        name: "candidate-lut16",
        lower: lut16::<false>,
        upper: Some(lut16::<true>),
        decode: ranges,
    },
    Codec {
        name: "candidate-pairs",
        lower: pairs::<false>,
        upper: Some(pairs::<true>),
        decode: lut_decode,
    },
    Codec {
        name: "candidate-arithmetic",
        lower: arithmetic::<false>,
        upper: Some(arithmetic::<true>),
        decode: lut_decode,
    },
    Codec {
        name: "candidate-swar",
        lower: swar::<false>,
        upper: Some(swar::<true>),
        decode: lut_decode,
    },
];

const LOWER: &[u8; 16] = b"0123456789abcdef";
const UPPER: &[u8; 16] = b"0123456789ABCDEF";
const fn pair_table(upper: bool) -> [[u8; 2]; 256] {
    let chars = if upper { UPPER } else { LOWER };
    let mut table = [[0; 2]; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = [chars[i >> 4], chars[i & 15]];
        i += 1;
    }
    table
}
const LOWER_PAIRS: [[u8; 2]; 256] = pair_table(false);
const UPPER_PAIRS: [[u8; 2]; 256] = pair_table(true);

fn lut16<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    let chars = if U { UPPER } else { LOWER };
    for (&b, out) in src.iter().zip(dst[..n].chunks_exact_mut(2)) {
        out[0] = chars[(b >> 4) as usize];
        out[1] = chars[(b & 15) as usize];
    }
    Ok(n)
}

fn pairs<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    let table = if U { &UPPER_PAIRS } else { &LOWER_PAIRS };
    for (&b, out) in src.iter().zip(dst[..n].chunks_exact_mut(2)) {
        out.copy_from_slice(&table[b as usize]);
    }
    Ok(n)
}

fn arithmetic<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    let offset = if U { 7 } else { 39 };
    for (&b, out) in src.iter().zip(dst[..n].chunks_exact_mut(2)) {
        let h = b >> 4;
        let l = b & 15;
        out[0] = h + b'0' + ((h > 9) as u8) * offset;
        out[1] = l + b'0' + ((l > 9) as u8) * offset;
    }
    Ok(n)
}

fn swar<const U: bool>(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, true)?;
    let offset = if U { 7 } else { 39 };
    let chunks = src.len() / 4;
    for (input, out) in src[..chunks * 4]
        .chunks_exact(4)
        .zip(dst[..chunks * 8].chunks_exact_mut(8))
    {
        let mut x = u32::from_le_bytes(input.try_into().unwrap()) as u64;
        x = (x | x << 16) & 0x0000_ffff_0000_ffff;
        x = (x | x << 8) & 0x00ff_00ff_00ff_00ff;
        let nibbles = ((x >> 4) & 0x000f_000f_000f_000f) | ((x & 0x000f_000f_000f_000f) << 8);
        let letters = ((nibbles + 0x0606_0606_0606_0606) & 0x1010_1010_1010_1010) >> 4;
        let ascii = nibbles + 0x3030_3030_3030_3030 + letters * offset;
        out.copy_from_slice(&ascii.to_le_bytes());
    }
    lut16::<U>(&src[chunks * 4..], &mut dst[chunks * 8..n])?;
    Ok(n)
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn ranges(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    for (i, (pair, out)) in src.chunks_exact(2).zip(dst[..n].iter_mut()).enumerate() {
        let hi = nibble(pair[0]).ok_or(Error::InvalidByte {
            index: i * 2,
            byte: pair[0],
        })?;
        let lo = nibble(pair[1]).ok_or(Error::InvalidByte {
            index: i * 2 + 1,
            byte: pair[1],
        })?;
        *out = (hi << 4) | lo;
    }
    Ok(n)
}

const fn decode_table() -> [u8; 256] {
    let mut out = [255; 256];
    let mut i = 0;
    while i < 10 {
        out[b'0' as usize + i] = i as u8;
        i += 1;
    }
    i = 0;
    while i < 6 {
        out[b'a' as usize + i] = i as u8 + 10;
        out[b'A' as usize + i] = i as u8 + 10;
        i += 1;
    }
    out
}
const DECODE: [u8; 256] = decode_table();

fn lut_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    for (i, (pair, out)) in src.chunks_exact(2).zip(dst[..n].iter_mut()).enumerate() {
        let hi = DECODE[pair[0] as usize];
        let lo = DECODE[pair[1] as usize];
        if hi | lo == 255 {
            let index = i * 2 + usize::from(hi != 255);
            return Err(Error::InvalidByte {
                index,
                byte: src[index],
            });
        }
        *out = (hi << 4) | lo;
    }
    Ok(n)
}
