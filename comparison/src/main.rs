mod adapters;
mod candidates;
mod kernels;
#[path = "previous/lib.rs"]
#[allow(clippy::chunks_exact_to_as_chunks, clippy::manual_is_multiple_of)]
mod previous;

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2"
))]
#[path = "../../src/sse2.rs"]
mod sse2;

// Include the production implementations to isolate kernel/dispatch choices
// without exposing benchmark-only APIs or dependencies in the released crate.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[path = "../../src/neon.rs"]
mod neon;
#[path = "../../src/scalar.rs"]
#[allow(clippy::chunks_exact_to_as_chunks)]
mod scalar;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[path = "../../src/x86.rs"]
mod x86;

use adapters::{Codec, Convert};
use std::hint::black_box;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use zhex::Error;

const PRIMARY: &[usize] = &[16, 32, 64, 256, 4096, 65536, 1048576, 8388608];
const OFFSETS: &[(usize, usize)] = &[(0, 0), (1, 3), (7, 15), (31, 1)];
const SEED: u64 = 0x4d59_5df4_d0f3_3173;

#[derive(Clone, Copy, PartialEq)]
enum Operation {
    Lower,
    Upper,
    Decode,
    InvalidFirst,
    InvalidMiddle,
    InvalidLast,
}
impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Lower => "encode_lower",
            Self::Upper => "encode_upper",
            Self::Decode => "decode_mixed",
            Self::InvalidFirst => "invalid_first",
            Self::InvalidMiddle => "invalid_middle",
            Self::InvalidLast => "invalid_last",
        }
    }
    fn encode(self) -> bool {
        matches!(self, Self::Lower | Self::Upper)
    }
    fn invalid(self) -> bool {
        matches!(
            self,
            Self::InvalidFirst | Self::InvalidMiddle | Self::InvalidLast
        )
    }
    fn function(self, codec: Codec) -> Option<Convert> {
        match self {
            Self::Lower => Some(codec.lower),
            Self::Upper => codec.upper,
            _ => Some(codec.decode),
        }
    }
}

/// Align the tested slice, independently of the allocator's chosen base address.
struct Buffer {
    data: Vec<u8>,
    start: usize,
    len: usize,
}
impl Buffer {
    fn new(len: usize, offset: usize) -> Self {
        let data = vec![0xa5; len + 128];
        let start = (64 - data.as_ptr() as usize % 64) % 64 + offset;
        Self { data, start, len }
    }
    fn get(&self) -> &[u8] {
        &self.data[self.start..self.start + self.len]
    }
    fn get_mut(&mut self) -> &mut [u8] {
        &mut self.data[self.start..self.start + self.len]
    }
    fn guards(&self) {
        assert!(self.data[..self.start]
            .iter()
            .chain(&self.data[self.start + self.len..])
            .all(|&b| b == 0xa5));
    }
}

fn bytes(len: usize) -> Vec<u8> {
    let mut state = SEED;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

fn reference_encode(src: &[u8], upper: bool) -> Vec<u8> {
    let alphabet = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    src.iter()
        .flat_map(|&b| [alphabet[(b / 16) as usize], alphabet[(b % 16) as usize]])
        .collect()
}

fn reference_decode(src: &[u8], capacity: usize) -> Result<Vec<u8>, Error> {
    if src.len() & 1 != 0 {
        return Err(Error::OddLength);
    }
    let required = src.len() / 2;
    if capacity < required {
        return Err(Error::OutputTooSmall {
            required,
            available: capacity,
        });
    }
    let mut out = vec![0; required];
    for (index, &byte) in src.iter().enumerate() {
        let value = (byte as char)
            .to_digit(16)
            .ok_or(Error::InvalidByte { index, byte })? as u8;
        out[index / 2] = out[index / 2] * 16 + value;
    }
    Ok(out)
}

fn verify_adapters(codecs: &[Codec]) {
    for &codec in codecs {
        let input = (0..=255).collect::<Vec<u8>>();
        for (function, upper) in [(Some(codec.lower), false), (codec.upper, true)] {
            if let Some(f) = function {
                let mut dst = vec![0xa5; 513];
                assert_eq!(f(&input, &mut dst), Ok(512), "{} encode", codec.name);
                assert_eq!(&dst[..512], reference_encode(&input, upper));
                assert_eq!(dst[512], 0xa5);
                let mut short = [0xa5; 1];
                assert_eq!(
                    f(b"a", &mut short),
                    Err(Error::OutputTooSmall {
                        required: 2,
                        available: 1
                    })
                );
                assert_eq!(short, [0xa5]);
            }
        }
        for hi in 0..=255_u8 {
            for lo in 0..=255_u8 {
                let src = [hi, lo];
                let expected = reference_decode(&src, 1);
                let mut dst = [0xa5; 2];
                assert_eq!(
                    (codec.decode)(&src, &mut dst),
                    expected.as_ref().map(|v| v.len()).map_err(|e| *e),
                    "{} decode {src:?}",
                    codec.name
                );
                if let Ok(data) = expected {
                    assert_eq!(dst[0], data[0]);
                }
                assert_eq!(dst[1], 0xa5);
            }
        }
        for src in [
            b"?".as_slice(),
            b"0x",
            b"0x00",
            b" 0",
            b"ff\n0",
            b"abcd",
            b"",
        ] {
            for capacity in 0..=src.len() / 2 + 1 {
                let mut dst = vec![0xa5; capacity];
                let expected = reference_decode(src, capacity);
                let actual = (codec.decode)(src, &mut dst);
                assert_eq!(
                    actual,
                    expected.as_ref().map(|x| x.len()).map_err(|e| *e),
                    "{} preflight {src:?}",
                    codec.name
                );
                if src.len() % 2 == 1 || capacity < src.len() / 2 {
                    assert!(dst.iter().all(|&b| b == 0xa5));
                }
            }
        }
    }
    eprintln!(
        "Validated {} adapters against independent reference, including all 65,536 byte pairs.",
        codecs.len()
    );
}

fn time_loop(f: Convert, src: &[u8], dst: &mut [u8], iterations: u64) -> u128 {
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = black_box(black_box(f)(black_box(src), black_box(&mut *dst)));
    }
    start.elapsed().as_nanos()
}

fn calibrate(f: Convert, src: &[u8], dst: &mut [u8], warmup: Duration, target: Duration) -> u64 {
    let mut count = 1;
    loop {
        let elapsed = time_loop(f, src, dst, count);
        if elapsed >= warmup.as_nanos() {
            return ((target.as_nanos() * count as u128) / elapsed.max(1)).clamp(1, 100_000_000)
                as u64;
        }
        count *= 2;
    }
}

struct Settings {
    smoke: bool,
    primary: bool,
    candidates: bool,
    check_only: bool,
    kernels: bool,
    tune: bool,
    short: bool,
}

fn main() -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    for arg in &args {
        assert!(
            [
                "--smoke",
                "--primary",
                "--candidates",
                "--check-only",
                "--kernels",
                "--tune",
                "--short"
            ]
            .contains(&arg.as_str()),
            "unknown argument: {arg}"
        );
    }
    let settings = Settings {
        smoke: args.iter().any(|a| a == "--smoke"),
        primary: args.iter().any(|a| a == "--primary"),
        candidates: args.iter().any(|a| a == "--candidates"),
        check_only: args.iter().any(|a| a == "--check-only"),
        kernels: args.iter().any(|a| a == "--kernels"),
        tune: args.iter().any(|a| a == "--tune"),
        short: args.iter().any(|a| a == "--short"),
    };
    assert!(
        !(settings.kernels && settings.candidates),
        "choose candidates or kernels"
    );
    let codecs = if settings.tune {
        vec![adapters::CODECS[0], adapters::PREVIOUS]
    } else if settings.kernels {
        [vec![adapters::CODECS[0]], kernels::codecs()].concat()
    } else if settings.candidates {
        [vec![adapters::CODECS[0]], candidates::CODECS.to_vec()].concat()
    } else {
        [adapters::CODECS.to_vec(), vec![adapters::PREVIOUS]].concat()
    };
    verify_adapters(&codecs);
    if settings.check_only {
        return Ok(());
    }
    let sizes = if settings.smoke {
        vec![0, 1, 15, 16, 31, 32, 63, 64, 65, 256, 4096]
    } else if settings.short {
        (0..=64).collect()
    } else if settings.primary {
        PRIMARY.to_vec()
    } else {
        let mut sizes = (0..=64)
            .chain([65, 95, 96, 97, 127, 128, 129, 255, 256, 257, 511, 512, 513])
            .chain(PRIMARY.iter().copied())
            .collect::<Vec<_>>();
        sizes.sort_unstable();
        sizes.dedup();
        sizes
    };
    let offsets = if settings.smoke {
        &OFFSETS[..2]
    } else {
        OFFSETS
    };
    let samples = if settings.smoke { 2 } else { 7 };
    let warmup = Duration::from_micros(if settings.smoke { 20 } else { 1000 });
    let target = Duration::from_micros(if settings.smoke { 50 } else { 2000 });
    let build = std::env::var("ZHEX_BENCH_BUILD").unwrap_or_else(|_| "unspecified".into());
    let stdout = io::stdout();
    let mut csv = io::BufWriter::new(stdout.lock());
    writeln!(csv, "build,group,operation,payload_bytes,input_offset,output_offset,implementation,status,sample,iterations,elapsed_ns,ns_per_op,payload_gib_s")?;
    for n in sizes {
        let payload = bytes(n);
        let lower = reference_encode(&payload, false);
        let upper = reference_encode(&payload, true);
        let mut mixed = lower.clone();
        for (i, b) in mixed.iter_mut().enumerate() {
            if i % 2 == 0 {
                b.make_ascii_uppercase();
            }
        }
        for operation in [
            Operation::Lower,
            Operation::Upper,
            Operation::Decode,
            Operation::InvalidFirst,
            Operation::InvalidMiddle,
            Operation::InvalidLast,
        ] {
            if operation.invalid() && (n == 0 || (!settings.smoke && !PRIMARY.contains(&n))) {
                continue;
            }
            let mut source = if operation.encode() {
                payload.clone()
            } else {
                mixed.clone()
            };
            let invalid_index = match operation {
                Operation::InvalidFirst => Some(0),
                Operation::InvalidMiddle => Some(source.len() / 2),
                Operation::InvalidLast => Some(source.len() - 1),
                _ => None,
            };
            if let Some(i) = invalid_index {
                source[i] = 0x80;
            }
            let expected = match operation {
                Operation::Lower => &lower,
                Operation::Upper => &upper,
                _ => &payload,
            };
            let result = invalid_index.map_or(Ok(expected.len()), |index| {
                Err(Error::InvalidByte { index, byte: 0x80 })
            });
            let group = if operation.invalid() {
                "invalid"
            } else if PRIMARY.contains(&n) {
                "primary"
            } else {
                "coverage"
            };
            for &(input_offset, output_offset) in offsets {
                let mut src = Buffer::new(source.len(), input_offset);
                src.get_mut().copy_from_slice(&source);
                let mut dst = Buffer::new(expected.len(), output_offset);
                let mut counts = Vec::new();
                for &codec in &codecs {
                    if let Some(f) = operation.function(codec) {
                        // Every valid output byte starts wrong. A codec that
                        // skips stores cannot inherit another codec's result.
                        for (byte, &want) in dst.get_mut().iter_mut().zip(expected) {
                            *byte = if result.is_ok() { want ^ 0xff } else { 0xa5 };
                        }
                        assert_eq!(
                            f(src.get(), dst.get_mut()),
                            result,
                            "{} {} {n}",
                            codec.name,
                            operation.name()
                        );
                        if result.is_ok() {
                            assert_eq!(
                                dst.get(),
                                expected,
                                "{} {} {n}",
                                codec.name,
                                operation.name()
                            );
                        }
                        src.guards();
                        dst.guards();
                        counts.push(Some(calibrate(f, src.get(), dst.get_mut(), warmup, target)));
                    } else {
                        counts.push(None);
                        writeln!(csv, "{build},{group},{},{n},{input_offset},{output_offset},{},unsupported,,,,,", operation.name(), codec.name)?;
                    }
                }
                for sample in 0..samples {
                    for j in 0..codecs.len() {
                        let i = (j + sample) % codecs.len();
                        let codec = codecs[i];
                        if let (Some(f), Some(count)) = (operation.function(codec), counts[i]) {
                            let elapsed = time_loop(f, src.get(), dst.get_mut(), count);
                            assert_eq!(f(src.get(), dst.get_mut()), result);
                            if result.is_ok() {
                                assert_eq!(dst.get(), expected);
                            }
                            dst.guards();
                            let ns = elapsed as f64 / count as f64;
                            let throughput = if operation.invalid() {
                                String::new()
                            } else {
                                format!("{:.6}", n as f64 / ns * 1e9 / 1073741824.0)
                            };
                            writeln!(csv, "{build},{group},{},{n},{input_offset},{output_offset},{},ok,{sample},{count},{elapsed},{ns:.6},{throughput}", operation.name(), codec.name)?;
                        }
                    }
                }
            }
        }
        eprintln!("finished payload {n} bytes");
    }
    csv.flush()
}
