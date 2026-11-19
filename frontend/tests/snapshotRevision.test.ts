import { expect, test } from "bun:test";
import { SnapshotRevision } from "../app/utils/snapshotRevision";

test("an HTTP response cannot overwrite a newer live update", () => {
  const revisions = new SnapshotRevision();
  const first = revisions.capture();
  revisions.invalidate();
  expect(revisions.isCurrent(first)).toBe(false);
  const second = revisions.capture();
  expect(revisions.isCurrent(second)).toBe(true);
  revisions.invalidate();
  expect(revisions.isCurrent(second)).toBe(false);
});
