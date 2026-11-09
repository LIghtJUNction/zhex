use crate::Error;

#[inline]
pub(crate) fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
    let alphabet = if UPPER {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    for (&byte, pair) in input.iter().zip(output.chunks_exact_mut(2)) {
        pair[0] = alphabet[(byte >> 4) as usize];
        pair[1] = alphabet[(byte & 15) as usize];
    }
}

#[inline]
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[inline]
pub(crate) fn decode(input: &[u8], output: &mut [u8], offset: usize) -> Result<(), Error> {
    for (i, (pair, byte)) in input.chunks_exact(2).zip(output).enumerate() {
        let high = nibble(pair[0]).ok_or(Error::InvalidByte {
            index: offset + 2 * i,
            byte: pair[0],
        })?;
        let low = nibble(pair[1]).ok_or(Error::InvalidByte {
            index: offset + 2 * i + 1,
            byte: pair[1],
        })?;
        *byte = high << 4 | low;
    }
    Ok(())
}
