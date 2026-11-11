use zhex::{Error, decode, decoded_len, encode_lower, encode_upper, encoded_len};

// Include the production kernels so an AVX2-capable host can still execute the
// SSE2 fallback at lengths above the public AVX2 dispatch threshold.
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_env = "sgx"),
    not(feature = "force-scalar"),
    not(miri)
))]
#[path = "../src/scalar.rs"]
#[allow(dead_code)] // The decoder's tail dependency also contains the encoder.
mod scalar;

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_env = "sgx"),
    not(feature = "force-scalar"),
    not(miri)
))]
#[path = "../src/sse2.rs"]
mod sse2;

const SENTINEL: u8 = 0xa5;

// Deliberately simple, independent references: no tables, packed operations, or
// zhex calls. These also specify the error ordering for differential checks.
fn reference_encode(input: &[u8], upper: bool) -> Vec<u8> {
    input
        .iter()
        .flat_map(|&byte| [byte >> 4, byte & 15])
        .map(|n| match n {
            0..=9 => b'0' + n,
            _ if upper => b'A' + n - 10,
            _ => b'a' + n - 10,
        })
        .collect()
}

fn reference_decode(input: &[u8]) -> Result<Vec<u8>, Error> {
    if !input.len().is_multiple_of(2) {
        return Err(Error::OddLength);
    }
    let mut result = Vec::with_capacity(input.len() / 2);
    let mut high = 0;
    for (index, &byte) in input.iter().enumerate() {
        let value = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(Error::InvalidByte { index, byte }),
        };
        if index % 2 == 0 {
            high = value * 16;
        } else {
            result.push(high + value);
        }
    }
    Ok(result)
}

fn bytes(len: usize) -> Vec<u8> {
    let mut state = 0x8365_73e9_15ad_8c21_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

fn assert_encoding(input: &[u8]) {
    for upper in [false, true] {
        let expected = reference_encode(input, upper);
        let mut output = vec![SENTINEL; expected.len() + 7];
        let result = if upper {
            encode_upper(input, &mut output)
        } else {
            encode_lower(input, &mut output)
        };
        assert_eq!(result, Ok(expected.len()));
        assert_eq!(&output[..expected.len()], expected);
        assert_eq!(&output[expected.len()..], &[SENTINEL; 7]);
    }
}

fn assert_decoding(input: &[u8]) {
    let mut output = vec![SENTINEL; input.len() / 2 + 7];
    let expected = reference_decode(input);
    let result = decode(input, &mut output);
    match expected {
        Ok(expected) => {
            assert_eq!(result, Ok(expected.len()));
            assert_eq!(&output[..expected.len()], expected);
        }
        Err(error) => assert_eq!(result, Err(error)),
    }
    assert_eq!(&output[input.len() / 2..], &[SENTINEL; 7]);
}

#[test]
fn empty_input_and_oversized_output() {
    assert_eq!(encoded_len(0), Ok(0));
    assert_eq!(decoded_len(0), Ok(0));
    assert_eq!(encode_lower(&[], &mut []), Ok(0));
    assert_eq!(encode_upper(&[], &mut []), Ok(0));
    assert_eq!(decode(&[], &mut []), Ok(0));
    assert_encoding(&[]);
    assert_decoding(&[]);
}

#[test]
fn length_calculation_is_overflow_safe() {
    for n in 0..=128 {
        assert_eq!(encoded_len(n), Ok(n * 2));
        assert_eq!(decoded_len(n * 2), Ok(n));
        assert_eq!(decoded_len(n * 2 + 1), Err(Error::OddLength));
    }
    let largest = usize::MAX / 2;
    assert_eq!(encoded_len(largest), Ok(usize::MAX - 1));
    assert_eq!(encoded_len(largest + 1), Err(Error::LengthOverflow));
    assert_eq!(encoded_len(usize::MAX), Err(Error::LengthOverflow));
    assert_eq!(decoded_len(usize::MAX - 1), Ok(largest));
    assert_eq!(decoded_len(usize::MAX), Err(Error::OddLength));
}

#[test]
fn every_single_byte_and_both_case_alphabets() {
    for byte in 0..=u8::MAX {
        assert_encoding(&[byte]);
        assert_decoding(&reference_encode(&[byte], false));
        assert_decoding(&reference_encode(&[byte], true));
    }
    assert_decoding(b"0123456789aBcDeFABCDEFabcdef");
    assert_decoding(b"dEaDbeEF01020304");
}

#[test]
fn vector_blocks_and_tails_cover_every_byte_value() {
    // The single-byte/pair tests exercise scalar conversion. These lengths
    // exercise the short decoder, AVX2 encoding blocks and 16-byte tails.
    let values: Vec<u8> = if cfg!(miri) {
        vec![0, 9, 10, 15, 16, 127, 128, 240, 255]
    } else {
        (0..=u8::MAX).collect()
    };
    for byte in values {
        for len in [8, 9, 15, 16, 17, 31, 32, 33, 40, 47, 48, 49, 63] {
            let input = vec![byte; len];
            assert_encoding(&input);
            let lower = reference_encode(&input, false);
            let upper = reference_encode(&input, true);
            assert_decoding(&lower);
            assert_decoding(&upper);
            let mixed: Vec<u8> = lower
                .iter()
                .zip(&upper)
                .enumerate()
                .map(|(i, (&low, &high))| if i % 3 == 0 { low } else { high })
                .collect();
            assert_decoding(&mixed);
        }
    }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_env = "sgx"),
    not(feature = "force-scalar"),
    not(miri)
))]
#[test]
fn sse2_decoding_matches_reference_without_avx2_dispatch() {
    for len in 0..=128 {
        let payload = bytes(len);
        for source_offset in [0, 1, 7, 15] {
            for output_offset in [0, 1, 7, 15] {
                for upper in [false, true] {
                    let expected = reference_encode(&payload, upper);
                    let mut text = vec![SENTINEL; source_offset];
                    text.extend_from_slice(&expected);
                    let mut decoded = vec![SENTINEL; output_offset + len + 3];
                    // SAFETY: cfg guarantees SSE2, text has an even length and
                    // the disjoint exclusive output is exactly half that size.
                    // The kernel accepts these byte-aligned slices without padding.
                    assert_eq!(
                        unsafe {
                            sse2::decode(
                                &text[source_offset..],
                                &mut decoded[output_offset..output_offset + len],
                                0,
                            )
                        },
                        Ok(())
                    );
                    assert_eq!(&decoded[output_offset..output_offset + len], payload);
                    assert!(decoded[..output_offset].iter().all(|&b| b == SENTINEL));
                    assert_eq!(&decoded[output_offset + len..], &[SENTINEL; 3]);
                }
            }
        }
    }
    for len in [16, 18, 32, 34, 64, 66, 128, 130, 256] {
        for index in 0..len {
            let mut text = vec![b'A'; len];
            text[len - 1] = b'?';
            text[index] = 0x80;
            let mut output = vec![SENTINEL; len / 2];
            // SAFETY: compile-time SSE2, even text and half-sized independent
            // output. Alignment is unrestricted; offset 96 plus len cannot
            // overflow and models an AVX2 tail in the original input.
            assert_eq!(
                unsafe { sse2::decode(&text, &mut output, 96) },
                Err(Error::InvalidByte {
                    index: 96 + index,
                    byte: 0x80,
                })
            );
        }
    }
}

#[test]
fn all_two_byte_inputs_match_independent_reference() {
    let values: Vec<u8> = if cfg!(miri) {
        vec![0, 1, 9, 10, 15, 16, 127, 128, 254, 255]
    } else {
        (0..=u8::MAX).collect()
    };
    for &first in &values {
        for &second in &values {
            let input = [first, second];
            assert_encoding(&input);
            assert_decoding(&reference_encode(&input, false));
            assert_decoding(&reference_encode(&input, true));
        }
    }
}

#[test]
fn all_two_character_decodes_match_independent_reference() {
    let values: Vec<u8> = if cfg!(miri) {
        vec![
            0, b'/', b'0', b'9', b':', b'A', b'F', b'G', b'a', b'f', b'g', 255,
        ]
    } else {
        (0..=u8::MAX).collect()
    };
    for &first in &values {
        for &second in &values {
            assert_decoding(&[first, second]);
        }
    }
}

#[test]
fn preflight_errors_never_write_and_have_fixed_priority() {
    let mut output = [SENTINEL; 260];
    let input = bytes(130);
    for len in 1..=input.len() {
        let required = len * 2;
        let available = required - 1;
        let error = Err(Error::OutputTooSmall {
            required,
            available,
        });
        assert_eq!(encode_lower(&input[..len], &mut output[..available]), error);
        assert_eq!(output, [SENTINEL; 260]);
        assert_eq!(encode_upper(&input[..len], &mut output[..available]), error);
        assert_eq!(output, [SENTINEL; 260]);
    }
    let valid = [b'0'; 260];
    let invalid = [b'!'; 260];
    for len in 1..=130 {
        let error = Err(Error::OutputTooSmall {
            required: len,
            available: len - 1,
        });
        assert_eq!(decode(&invalid[..len * 2], &mut output[..len - 1]), error);
        assert_eq!(output, [SENTINEL; 260]);
        assert_eq!(decode(&valid[..len * 2], &mut output[..len - 1]), error);
        assert_eq!(output, [SENTINEL; 260]);
        assert_eq!(
            decode(&invalid[..len * 2 - 1], &mut output[..len - 1]),
            Err(Error::OddLength)
        );
        assert_eq!(output, [SENTINEL; 260]);
    }
    assert_eq!(decode(b"!", &mut []), Err(Error::OddLength));
    assert_eq!(decode(b"00!", &mut output), Err(Error::OddLength));
    assert_eq!(output, [SENTINEL; 260]);
}

#[test]
fn strict_ascii_syntax() {
    for input in [
        &b"0x00"[..],
        &b"0X00"[..],
        &b" 0"[..],
        &b"0 "[..],
        &b"\n0"[..],
        &b"0\t"[..],
        &b"00\r\n"[..],
        &b"0\0"[..],
        &b"\xc3\xa9"[..],
        &b"\xff\xff"[..],
    ] {
        assert!(reference_decode(input).is_err());
        assert_decoding(input);
    }
}

#[test]
fn every_invalid_byte_at_every_position_and_vector_boundary() {
    let lengths: &[usize] = if cfg!(miri) {
        &[2, 16, 34]
    } else {
        &[2, 4, 16, 18, 30, 32, 34, 62, 64, 66, 126, 128, 130, 258]
    };
    for &len in lengths {
        let mut input = reference_encode(&bytes(len / 2), false);
        for index in 0..len {
            let original = input[index];
            for byte in 0..=u8::MAX {
                if byte.is_ascii_hexdigit() || (cfg!(miri) && ![0, b'/', b'g', 255].contains(&byte))
                {
                    continue;
                }
                input[index] = byte;
                let mut output = vec![SENTINEL; len / 2 + 3];
                assert_eq!(
                    decode(&input, &mut output),
                    Err(Error::InvalidByte { index, byte }),
                    "length {len}, index {index}, byte {byte}"
                );
                assert_eq!(&output[len / 2..], &[SENTINEL; 3]);
            }
            input[index] = original;
        }
    }
}

#[test]
fn first_invalid_position_wins_across_chunks() {
    let len = if cfg!(miri) { 34 } else { 260 };
    let mut input = vec![b'f'; len];
    for first in 0..len - 1 {
        input[first] = b'?';
        for second in first + 1..len {
            input[second] = 0xff;
            let mut output = vec![SENTINEL; len / 2];
            assert_eq!(
                decode(&input, &mut output),
                Err(Error::InvalidByte {
                    index: first,
                    byte: b'?'
                })
            );
            input[second] = b'f';
        }
        input[first] = b'f';
    }
}

#[test]
fn error_positions_are_relative_to_the_input_slice() {
    let offsets: &[usize] = if cfg!(miri) {
        &[0, 1, 7]
    } else {
        &[0, 1, 7, 15, 31, 32, 63]
    };
    for &source_offset in offsets {
        let mut input = vec![b'a'; source_offset + 130];
        for index in [0, 1, 15, 16, 31, 32, 63, 64, 129] {
            input[source_offset + index] = 0xfd;
            for &output_offset in offsets {
                let mut output = vec![SENTINEL; output_offset + 65 + 3];
                assert_eq!(
                    decode(
                        &input[source_offset..],
                        &mut output[output_offset..output_offset + 65]
                    ),
                    Err(Error::InvalidByte { index, byte: 0xfd })
                );
                assert!(output[..output_offset].iter().all(|&b| b == SENTINEL));
                assert_eq!(&output[output_offset + 65..], &[SENTINEL; 3]);
            }
            input[source_offset + index] = b'a';
        }
    }
}

#[test]
fn independent_slice_offsets_and_exact_vector_tails() {
    let lengths: Vec<usize> = if cfg!(miri) {
        vec![0, 1, 7, 16, 17, 31, 32, 33]
    } else {
        (0..=65)
            .chain([127, 128, 129, 255, 256, 257, 511, 512, 513])
            .collect()
    };
    let offsets: Vec<usize> = if cfg!(miri) {
        vec![0, 1, 7]
    } else {
        (0..=32).collect()
    };
    for len in lengths {
        let input = bytes(len);
        let lower = reference_encode(&input, false);
        let upper = reference_encode(&input, true);
        let mixed: Vec<u8> = lower
            .iter()
            .zip(&upper)
            .enumerate()
            .map(|(index, (&low, &high))| if index % 3 == 0 { high } else { low })
            .collect();
        for &source_offset in &offsets {
            let mut source = vec![SENTINEL; source_offset];
            source.extend_from_slice(&input);
            let mut text = vec![SENTINEL; source_offset];
            text.extend_from_slice(&mixed);
            for &output_offset in &offsets {
                for (upper_case, expected) in [(false, &lower), (true, &upper)] {
                    let mut output = vec![SENTINEL; output_offset + len * 2 + 3];
                    let destination = &mut output[output_offset..output_offset + len * 2];
                    let actual = if upper_case {
                        encode_upper(&source[source_offset..], destination)
                    } else {
                        encode_lower(&source[source_offset..], destination)
                    };
                    assert_eq!(actual, Ok(len * 2));
                    assert_eq!(&output[output_offset..output_offset + len * 2], expected);
                    assert!(output[..output_offset].iter().all(|&b| b == SENTINEL));
                    assert_eq!(&output[output_offset + len * 2..], &[SENTINEL; 3]);
                }
                let mut output = vec![SENTINEL; output_offset + len + 3];
                assert_eq!(
                    decode(
                        &text[source_offset..],
                        &mut output[output_offset..output_offset + len]
                    ),
                    Ok(len)
                );
                assert_eq!(&output[output_offset..output_offset + len], input);
                assert!(output[..output_offset].iter().all(|&b| b == SENTINEL));
                assert_eq!(&output[output_offset + len..], &[SENTINEL; 3]);
            }
        }
    }
}

#[test]
fn large_input_matches_reference_and_preserves_error_offset() {
    let lengths: &[usize] = if cfg!(miri) {
        &[131]
    } else {
        &[4096, 65536, 1_048_576, 4_194_305]
    };
    for &len in lengths {
        let input = bytes(len);
        assert_encoding(&input);
        let mut encoded = reference_encode(&input, false);
        for (index, byte) in encoded.iter_mut().enumerate() {
            if index % 3 == 0 {
                *byte = byte.to_ascii_uppercase();
            }
        }
        assert_decoding(&encoded);
        let index = encoded.len() - 1;
        encoded[index] = 0xfe;
        let mut output = vec![SENTINEL; len];
        assert_eq!(
            decode(&encoded, &mut output),
            Err(Error::InvalidByte { index, byte: 0xfe })
        );
    }
}
