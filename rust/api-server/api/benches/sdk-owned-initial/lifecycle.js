// bun lifecycle.js /path/to/client: serve two v2 connections on an ephemeral loopback port.
import assert from "node:assert/strict";
const byte = n => Buffer.from([n]);
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b };
const u64 = n => { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(n)); return b };
const cat = (...parts) => Buffer.concat(parts);
const blob = b => cat(u32(b.length), b);
const name = blob(Buffer.from("location_state"));
const row = (id, x) => cat(u64(id), u64(123), u32(x), u32(3), u32(1));
const rows = (...rs) => cat(byte(0), Buffer.from([28, 0]), blob(cat(...rs))); // fixed-size BsatnRowList
const snapshot = rs => cat(u32(1), name, rs);
const tx = (qid, ins, del) => cat(u32(1), qid, u32(1), name, u32(1), byte(0), ins, del);
let opened = 0;
const server = Bun.serve({
  hostname: "127.0.0.1", port: 0,
  fetch(req, server) {
    if (!server.upgrade(req, {headers: {"sec-websocket-protocol": "v2.bsatn.spacetimedb"},
                              data: {round: ++opened}})) return new Response(null, {status: 400});
  },
  websocket: {
    open(ws) {
      ws.send(cat(byte(0), byte(0), Buffer.alloc(48), blob(Buffer.from("test"))));
    },
    message(ws, bytes) {
      const tag = bytes[0], req = bytes.subarray(1, 5);
      const send = (tag, ...parts) => ws.send(cat(byte(0), byte(tag), ...parts));
      if (tag === 0) {
        const qid = ws.data.qid = bytes.subarray(5, 9);
        // Reverse of regional callback order. The explicitly present stamina table is empty.
        const emptyStamina = cat(blob(Buffer.from("stamina_state")), byte(0), Buffer.from([20, 0]), u32(0));
        send(1, req, qid, u32(2), emptyStamina, name, ws.data.round === 1 ? rows() : rows(row(7, 0)));
        send(4, tx(qid, rows(row(17, 1)), rows()));
      } else if (tag === 3) {
        send(6, req, u64(1), byte(0), blob(Buffer.alloc(0)),
             tx(ws.data.qid, rows(row(17, 2)), rows(row(17, 1))));
      } else {
        assert.equal(tag, 1);
        const dropped = ws.data.round === 1 ? rows(row(17, 2)) : rows(row(7, 0), row(17, 2));
        send(2, req, ws.data.qid, byte(0), snapshot(dropped));
        ws.close(1000);
      }
    },
  },
});
try {
  const proc = Bun.spawn([process.argv[2], `ws://127.0.0.1:${server.port}`], {stdout: "inherit", stderr: "inherit", timeout: 15000});
  assert.equal(await proc.exited, 0);
  assert.equal(opened, 2);
} finally {
  server.stop(true);
}
