// Appended only to the private copy of the generated bindings. Time chunks
// small enough not to hold an entire capture's decoded rows at once.
pub fn replay_try_from_phases(frames: &[&[u8]], clock: fn() -> i64) -> [i64; 4] {
    use std::hint::black_box;
    use spacetimedb_sdk::unstable::streaming_decompression::streaming;
    type Raw = __ws::v2::TableUpdate;
    fn unpack(f: &[u8]) -> __ws::v2::TransactionUpdate {
        let __ws::v2::ServerMessage::TransactionUpdate(t) = streaming(f).unwrap() else { panic!() };
        t
    }
    let mut ns = [0; 4];
    for fs in frames.chunks(256) {
        let txs: Vec<_> = fs.iter().map(|f| unpack(f)).collect();
        let mut full = Vec::with_capacity(txs.len());
        let t = clock();
        for tx in txs { full.push(DbUpdate::try_from(black_box(tx)).unwrap()); }
        ns[0] += clock() - t;
        drop(black_box(full));

        let mut raw = Vec::<Raw>::new();
        let mut counts = Vec::new();
        for f in fs {
            let n = raw.len();
            raw.extend(__sdk::transaction_update_iter_table_updates(unpack(f)));
            counts.push(raw.len() - n);
        }
        let mut ids = Vec::with_capacity(raw.len());
        let t = clock();
        for table in &raw { ids.push(replay_table_id(black_box(&table.table_name[..]))); }
        ns[1] += clock() - t;
        let mut parsed = Vec::with_capacity(raw.len());
        let t = clock();
        for (id, table) in black_box(ids).into_iter().zip(raw) {
            parsed.push(replay_parse_id(id, table).unwrap());
        }
        ns[2] += clock() - t;
        let mut it = black_box(parsed).into_iter();
        let mut outputs = Vec::with_capacity(counts.len());
        let t = clock();
        for n in counts {
            let mut update = DbUpdate::with_table_capacity(n);
            for table in it.by_ref().take(n) { update.append(table); }
            outputs.push(update);
        }
        ns[3] += clock() - t;
        assert_eq!(it.len(), 0);
        drop(black_box(outputs));
    }
    ns
}
