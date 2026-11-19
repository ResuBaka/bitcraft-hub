/** Prevent an older HTTP response from overwriting a newer live update. */
export class SnapshotRevision {
  private revision = 0;

  invalidate() {
    this.revision++;
  }

  capture(): number {
    return this.revision;
  }

  isCurrent(revision: number): boolean {
    return revision === this.revision;
  }
}
