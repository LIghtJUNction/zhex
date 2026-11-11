#![cfg(not(miri))]

use std::sync::Barrier;
use zhex::{Error, decode, encode_lower, encode_upper};

// A separate integration binary ensures dispatch starts cold. Each worker owns
// its buffers; only the internal CPU-feature cache can be shared by the calls.
#[test]
fn concurrent_first_use_and_repeated_dispatch() {
    let start = Barrier::new(16);
    std::thread::scope(|scope| {
        for worker in 0..16 {
            let start = &start;
            scope.spawn(move || {
                let input: Vec<u8> = (0..257 + worker)
                    .map(|i| (i as u8).wrapping_mul(73).wrapping_add(worker as u8))
                    .collect();
                let lower: Vec<u8> = input
                    .iter()
                    .flat_map(|byte| format!("{byte:02x}").into_bytes())
                    .collect();
                let upper = lower.to_ascii_uppercase();
                let mut text = vec![0; lower.len()];
                let mut bytes = vec![0; input.len()];
                start.wait();
                for iteration in 0..128 {
                    let expected = if iteration % 2 == 0 {
                        assert_eq!(encode_lower(&input, &mut text), Ok(lower.len()));
                        &lower
                    } else {
                        assert_eq!(encode_upper(&input, &mut text), Ok(upper.len()));
                        &upper
                    };
                    assert_eq!(&text, expected);
                    assert_eq!(decode(expected, &mut bytes), Ok(input.len()));
                    assert_eq!(bytes, input);
                    let index = (iteration * 31 + worker) % text.len();
                    text[index] = b'!';
                    assert_eq!(
                        decode(&text, &mut bytes),
                        Err(Error::InvalidByte { index, byte: b'!' })
                    );
                }
            });
        }
    });
}
