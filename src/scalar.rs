use crate::Error;

#[inline]
pub(crate) fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
    let letter_offset = if UPPER { 7 } else { 39 };
    for (&byte, pair) in input.iter().zip(output.chunks_exact_mut(2)) {
        let high = byte >> 4;
        let low = byte & 15;
        pair[0] = high + b'0' + u8::from(high > 9) * letter_offset;
        pair[1] = low + b'0' + u8::from(low > 9) * letter_offset;
    }
}

const fn decode_table() -> [u8; 256] {
    let mut table = [255; 256];
    let mut i = 0;
    while i < 10 {
        table[b'0' as usize + i] = i as u8;
        i += 1;
    }
    i = 0;
    while i < 6 {
        table[b'a' as usize + i] = i as u8 + 10;
        table[b'A' as usize + i] = i as u8 + 10;
        i += 1;
    }
    table
}

const NIBBLES: [u8; 256] = decode_table();

#[inline]
pub(crate) fn decode(input: &[u8], output: &mut [u8], offset: usize) -> Result<(), Error> {
    for (i, (pair, byte)) in input.as_chunks::<2>().0.iter().zip(output).enumerate() {
        let high = NIBBLES[pair[0] as usize];
        let low = NIBBLES[pair[1] as usize];
        if high | low == 255 {
            let invalid = usize::from(high != 255);
            return Err(Error::InvalidByte {
                index: offset + 2 * i + invalid,
                byte: pair[invalid],
            });
        }
        *byte = high << 4 | low;
    }
    Ok(())
}
