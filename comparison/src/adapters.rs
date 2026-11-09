use hex_simd::AsOut;
use zhex::Error;

pub type Convert = fn(&[u8], &mut [u8]) -> Result<usize, Error>;

#[derive(Clone, Copy)]
pub struct Codec {
    pub name: &'static str,
    pub lower: Convert,
    pub upper: Option<Convert>,
    pub decode: Convert,
}

pub const CODECS: &[Codec] = &[
    Codec {
        name: "zhex",
        lower: zhex::encode_lower,
        upper: Some(zhex::encode_upper),
        decode: zhex::decode,
    },
    Codec {
        name: "hex",
        lower: hex_lower,
        upper: None,
        decode: hex_decode,
    },
    Codec {
        name: "faster-hex",
        lower: faster_lower,
        upper: Some(faster_upper),
        decode: faster_decode,
    },
    Codec {
        name: "const-hex",
        lower: const_lower,
        upper: Some(const_upper),
        decode: const_decode,
    },
    Codec {
        name: "hex-simd",
        lower: simd_lower,
        upper: Some(simd_upper),
        decode: simd_decode,
    },
    Codec {
        name: "better-hex",
        lower: better_lower,
        upper: Some(better_upper),
        decode: better_decode,
    },
    Codec {
        name: "fashex",
        lower: fashex_lower,
        upper: Some(fashex_upper),
        decode: fashex_decode,
    },
    Codec {
        name: "hex-turbo",
        lower: turbo_lower,
        upper: Some(turbo_upper),
        decode: turbo_decode,
    },
];

pub const PREVIOUS: Codec = Codec {
    name: "previous-zhex",
    lower: previous_lower,
    upper: Some(previous_upper),
    decode: previous_decode,
};

fn previous_result(result: Result<usize, crate::previous::Error>) -> Result<usize, Error> {
    result.map_err(|e| match e {
        crate::previous::Error::LengthOverflow => Error::LengthOverflow,
        crate::previous::Error::OddLength => Error::OddLength,
        crate::previous::Error::OutputTooSmall {
            required,
            available,
        } => Error::OutputTooSmall {
            required,
            available,
        },
        crate::previous::Error::InvalidByte { index, byte } => Error::InvalidByte { index, byte },
    })
}

fn previous_lower(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    previous_result(crate::previous::encode_lower(src, dst))
}

fn previous_upper(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    previous_result(crate::previous::encode_upper(src, dst))
}

fn previous_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    previous_result(crate::previous::decode(src, dst))
}

#[inline]
pub fn preflight(src: &[u8], dst: &[u8], encode: bool) -> Result<usize, Error> {
    let required = if encode {
        zhex::encoded_len(src.len())?
    } else {
        zhex::decoded_len(src.len())?
    };
    if dst.len() < required {
        return Err(Error::OutputTooSmall {
            required,
            available: dst.len(),
        });
    }
    Ok(required)
}

#[cold]
pub fn first_invalid(src: &[u8]) -> Error {
    let index = src
        .iter()
        .position(|b| !b.is_ascii_hexdigit())
        .expect("baseline rejected valid hex");
    Error::InvalidByte {
        index,
        byte: src[index],
    }
}

macro_rules! encoder {
    ($name:ident, $call:expr) => {
        fn $name(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
            let n = preflight(src, dst, true)?;
            $call(src, &mut dst[..n]);
            Ok(n)
        }
    };
}

encoder!(hex_lower, |s, d| {
    hex::encode_to_slice(s, d).unwrap();
});
encoder!(faster_lower, |s, d| {
    faster_hex::hex_encode(s, d).unwrap();
});
encoder!(faster_upper, |s, d| {
    faster_hex::hex_encode_upper(s, d).unwrap();
});
encoder!(const_lower, |s, d| {
    const_hex::encode_to_slice(s, d).unwrap();
});
encoder!(const_upper, |s, d| {
    const_hex::encode_to_slice_upper(s, d).unwrap();
});
encoder!(simd_lower, |s, d: &mut [u8]| {
    let _ = hex_simd::encode(s, d.as_out(), hex_simd::AsciiCase::Lower);
});
encoder!(simd_upper, |s, d: &mut [u8]| {
    let _ = hex_simd::encode(s, d.as_out(), hex_simd::AsciiCase::Upper);
});
encoder!(better_lower, |s, d| {
    better_hex::encode_to_slice(s, d).unwrap();
});
encoder!(better_upper, |s, d| {
    better_hex::encode_to_slice_upper(s, d).unwrap();
});
encoder!(fashex_lower, |s, d: &mut [u8]| {
    fashex::encode::<false>(s, d).unwrap();
});
encoder!(fashex_upper, |s, d: &mut [u8]| {
    fashex::encode::<true>(s, d).unwrap();
});
encoder!(turbo_lower, |s, d| {
    hex_turbo::LOWER_CASE.encode_slice(s, d).unwrap();
});
encoder!(turbo_upper, |s, d| {
    hex_turbo::UPPER_CASE.encode_slice(s, d).unwrap();
});

fn hex_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    hex::decode_to_slice(src, &mut dst[..n]).map_err(|e| match e {
        hex::FromHexError::InvalidHexCharacter { c, index } => Error::InvalidByte {
            index,
            byte: c as u8,
        },
        _ => panic!("unexpected preflight error: {e}"),
    })?;
    Ok(n)
}

fn faster_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    faster_hex::hex_decode(src, &mut dst[..n]).map_err(|e| match e {
        faster_hex::Error::InvalidChar { index, byte, .. } => Error::InvalidByte { index, byte },
        _ => panic!("unexpected preflight error: {e}"),
    })?;
    Ok(n)
}

fn const_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
    let n = preflight(src, dst, false)?;
    if src.starts_with(b"0x") || src.starts_with(b"0X") {
        return Err(Error::InvalidByte {
            index: 1,
            byte: src[1],
        });
    }
    const_hex::decode_to_slice(src, &mut dst[..n]).map_err(|e| match e {
        const_hex::FromHexError::InvalidHexCharacter { c, index } => Error::InvalidByte {
            index,
            byte: c as u8,
        },
        _ => panic!("unexpected preflight error: {e}"),
    })?;
    Ok(n)
}

macro_rules! decoder {
    ($name:ident, $call:expr) => {
        fn $name(src: &[u8], dst: &mut [u8]) -> Result<usize, Error> {
            let n = preflight(src, dst, false)?;
            if !$call(src, &mut dst[..n]) {
                return Err(first_invalid(src));
            }
            Ok(n)
        }
    };
}

decoder!(simd_decode, |s, d: &mut [u8]| hex_simd::decode(
    s,
    d.as_out()
)
.is_ok());
decoder!(better_decode, |s, d| better_hex::decode_to_slice(s, d)
    .is_ok());
decoder!(fashex_decode, |s, d: &mut [u8]| fashex::decode(s, d)
    .is_ok());
decoder!(turbo_decode, |s, d| hex_turbo::LOWER_CASE
    .decode_slice(s, d)
    .is_ok());
