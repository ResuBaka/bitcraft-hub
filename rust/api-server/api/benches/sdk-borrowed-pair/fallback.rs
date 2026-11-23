// Select intact transactions that never hit the live singleton-pair path.
// Keep their wire order; repeat that subsequence to give it a stable timing window.
use spacetimedb_sdk::{
    __codegen::__ws::v2::{ServerMessage, TableUpdateRows},
    unstable::streaming_decompression::streaming,
};
use std::{collections::HashMap, env, fs, io::Write, os::unix::fs::OpenOptionsExt};

fn main() {
    let input = fs::read(env::args_os().nth(1).unwrap()).unwrap();
    let mut src = &input[..];
    let mut subset = Vec::new();
    let mut total = 0usize;
    let mut selected = 0usize;
    let mut table_changes = 0usize;
    let mut pairs = 0usize;
    while !src.is_empty() {
        let len = usize::try_from(u64::from_le_bytes(src[1..9].try_into().unwrap())).unwrap();
        let (record, next) = src.split_at(9 + len);
        src = next;
        if record[0] != 4 || record[9] != 0 {
            continue;
        }
        let ServerMessage::TransactionUpdate(tx) = streaming(&record[9..]).unwrap() else {
            panic!("expected transaction");
        };
        total += 1;
        let mut counts = HashMap::new();
        for set in &tx.query_sets {
            for table in &set.tables {
                let counts = counts.entry(table.table_name.as_ref()).or_insert((0, 0));
                for rows in &table.rows {
                    match rows {
                        TableUpdateRows::PersistentTable(rows) => {
                            counts.0 += rows.deletes.iter_slices().count();
                            counts.1 += rows.inserts.iter_slices().count();
                        }
                        TableUpdateRows::EventTable(rows) => {
                            counts.1 += rows.events.iter_slices().count()
                        }
                    }
                }
            }
        }
        table_changes += counts.len();
        let in_frame = counts.values().filter(|&&n| n == (1, 1)).count();
        pairs += in_frame;
        if in_frame == 0 {
            subset.extend_from_slice(record);
            selected += 1;
        }
    }
    let repeats = total.div_ceil(selected);
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .mode(0o600)
        .write(true)
        .open(env::args_os().nth(2).unwrap())
        .unwrap();
    for _ in 0..repeats {
        output.write_all(&subset).unwrap();
    }
    println!(
        "transactions={total} tables={table_changes} pairs={pairs} fallback={selected} repeats={repeats}"
    );
}
