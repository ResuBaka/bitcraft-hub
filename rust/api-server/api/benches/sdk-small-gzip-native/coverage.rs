use spacetimedb_sdk::unstable::streaming_decompression::ReplayDecoder;
fn main() {
    let file = std::fs::read(std::env::args_os().nth(1).unwrap()).unwrap();
    let mut src = &file[..];
    let mut dec = ReplayDecoder::default();
    let (mut small, mut hit, mut spill, mut other) = (0, 0, 0, 0);
    while !src.is_empty() {
        let n = u64::from_le_bytes(src[1..9].try_into().unwrap()) as usize;
        let (wire, rest) = src[9..].split_at(n);
        src = rest;
        if n <= 4096 && wire[0] == 2 {
            small += 1;
            if dec.hit(wire) {
                hit += 1;
            } else if u32::from_le_bytes(*wire.last_chunk().unwrap()) >= 8192 {
                spill += 1;
            } else {
                other += 1;
            }
        } else {
            assert!(!dec.hit(wire));
        }
    }
    println!(
        r#"{{"small_gzip":{small},"hit":{hit},"size_hint_spill":{spill},"other_fallback":{other}}}"#
    );
}
