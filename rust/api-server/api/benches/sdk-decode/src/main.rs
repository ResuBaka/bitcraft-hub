use bytes::Bytes;
use flate2::bufread::GzDecoder;
use game_module::module_bindings::DbUpdate;
use spacetimedb_sdk::{
    __codegen::{__sats::bsatn, __ws::v2::ServerMessage, DbUpdate as _},
    unstable::streaming_decompression as sdk,
};
use std::{
    env, fs,
    hint::black_box,
    io::{Cursor, Read},
};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Stock,
    SliceInput,
    Reset,
    Small,
    Stateless,
}

// Model a decoder owned by one connection. Drop the input reference after
// every message (including on error) without retaining a large last frame.
struct Decoder {
    gzip: GzDecoder<Cursor<Bytes>>,
    scratch: Box<[u8; 8192]>,
}
impl Decoder {
    fn new() -> Self {
        Self {
            gzip: GzDecoder::new(Cursor::new(Bytes::new())),
            scratch: Box::new([0; 8192]),
        }
    }

    fn decode(&mut self, mode: Mode, wire: &Bytes) -> Result<ServerMessage, String> {
        if mode == Mode::Stock || wire.first() != Some(&2) {
            return sdk::streaming(wire);
        }
        if mode == Mode::SliceInput {
            return sdk::from_reader(GzDecoder::new(&wire[1..]), "gzip");
        }
        if mode == Mode::Stateless {
            let mut gz = GzDecoder::new(&wire[1..]);
            if wire.len() <= 4096 {
                return Self::small(&mut gz, &mut [0; 8192]);
            }
            return sdk::from_reader(gz, "gzip");
        }
        self.gzip.reset(Cursor::new(wire.slice(1..)));
        let result = if mode == Mode::Small && wire.len() <= 4096 {
            Self::small(&mut self.gzip, &mut self.scratch)
        } else {
            sdk::from_reader(&mut self.gzip, "gzip")
        };
        *self.gzip.get_mut() = Cursor::new(Bytes::new());
        result
    }

    fn small(mut gzip: impl Read, scratch: &mut [u8; 8192]) -> Result<ServerMessage, String> {
        let mut n = 0;
        while n < scratch.len() {
            match gzip.read(&mut scratch[n..]) {
                // First gzip member (including trailer) fully validated.
                Ok(0) => return bsatn::from_slice(&scratch[..n]).map_err(|e| e.to_string()),
                Ok(count) => n += count,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("gzip: {e}")),
            }
        }
        // No size trust, second inflate, or growing staging vector. The SDK
        // consumes the prefix and then drains the rest of the same member.
        sdk::from_reader(scratch.as_slice().chain(gzip), "gzip")
    }
}

struct Frame {
    tag: u8,
    wire: Bytes,
}

fn frames(path: &str, select: &str) -> Vec<Frame> {
    let mut bytes = Bytes::from(fs::read(path).unwrap());
    let mut frames = Vec::new();
    while !bytes.is_empty() {
        let hdr = bytes.split_to(9);
        let len = usize::try_from(u64::from_le_bytes(hdr[1..].try_into().unwrap())).unwrap();
        let wire = bytes.split_to(len);
        if match select {
            "all" => true,
            "gzip-tx" => hdr[0] == 4 && wire[0] == 2,
            "snap" => hdr[0] == 1,
            _ => panic!("bad selection"),
        } {
            frames.push(Frame { tag: hdr[0], wire });
        }
    }
    assert!(!frames.is_empty());
    frames
}

fn cpu_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) },
        0
    );
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

fn pass(dec: &mut Decoder, mode: Mode, frames: &[Frame], typed: bool) {
    for f in frames {
        let msg = dec.decode(mode, black_box(&f.wire)).unwrap();
        if typed {
            match msg {
                ServerMessage::SubscribeApplied(s) => {
                    drop(black_box(DbUpdate::parse_initial_rows(s.rows).unwrap()))
                }
                ServerMessage::TransactionUpdate(t) => {
                    drop(black_box(DbUpdate::try_from(t).unwrap()))
                }
                other => drop(black_box(other)),
            }
        } else {
            drop(black_box(msg));
        }
    }
}

fn check(frames: &[Frame]) {
    let mut dec = Decoder::new();
    for f in frames {
        let reference = bsatn::to_vec(&sdk::streaming(&f.wire).unwrap()).unwrap();
        for mode in [Mode::SliceInput, Mode::Reset, Mode::Small, Mode::Stateless] {
            let msg = dec.decode(mode, &f.wire).unwrap();
            assert!(
                bsatn::to_vec(&msg).unwrap() == reference,
                "decoded content differs"
            );
        }
    }
    let f = frames
        .iter()
        .find(|f| f.tag == 4 && f.wire[0] == 2)
        .unwrap();
    let valid = &f.wire;
    let mut corrupt = valid.to_vec();
    let crc = corrupt.len() - 8;
    corrupt[crc] ^= 1;
    for bad in [Bytes::from(corrupt), valid.slice(..valid.len() - 1)] {
        for mode in [
            Mode::Stock,
            Mode::SliceInput,
            Mode::Reset,
            Mode::Small,
            Mode::Stateless,
        ] {
            assert!(dec.decode(mode, &bad).is_err());
            assert!(dec.decode(mode, valid).is_ok());
        }
    }
    println!(
        "validated {} frames, corrupt/truncated trailer and recovery",
        frames.len()
    );
}

fn main() {
    let a: Vec<_> = env::args().collect();
    let mut frames = frames(&a[1], &a[3]);
    let mode = match a[2].as_str() {
        "check" => return check(&frames),
        "stock" => Mode::Stock,
        "none" => {
            // Model requesting Compression::None from the host. Construct the
            // alternate wire representation before warmup and CPU timing.
            for f in &mut frames {
                if f.wire[0] == 2 {
                    let mut plain = vec![0];
                    GzDecoder::new(&f.wire[1..])
                        .read_to_end(&mut plain)
                        .unwrap();
                    f.wire = plain.into();
                }
            }
            Mode::Stock
        }
        "slice" => Mode::SliceInput,
        "reset" => Mode::Reset,
        "small" => Mode::Small,
        "stateless" => Mode::Stateless,
        _ => panic!("bad mode"),
    };
    let mut dec = Decoder::new();
    let typed = match a[4].as_str() {
        "typed" => true,
        "envelope" => false,
        _ => panic!("bad phase"),
    };
    pass(&mut dec, mode, &frames, typed);
    let t = cpu_ns();
    pass(&mut dec, mode, &frames, typed);
    println!("{:.3}", (cpu_ns() - t) as f64 / 1e6);
}
