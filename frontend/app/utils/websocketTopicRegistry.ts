/** Tracks topic ownership separately from message handlers. */
export class WebsocketTopicRegistry {
  private owners = new Map<string, Set<string>>();
  private permanent = new Set<string>();

  private isUsed(topic: string): boolean {
    return (
      this.permanent.has(topic) || [...this.owners.values()].some((topics) => topics.has(topic))
    );
  }

  add(owner: string, topics: string[]): string[] {
    const owned = this.owners.get(owner) ?? new Set<string>();
    const added: string[] = [];
    for (const topic of topics) {
      if (!this.isUsed(topic)) added.push(topic);
      owned.add(topic);
      this.owners.set(owner, owned);
    }
    // Empty topic lists can still have a message handler (e.g. subscription acknowledgements).
    this.owners.set(owner, owned);
    return added;
  }

  addPermanent(topics: string[]): string[] {
    const added: string[] = [];
    for (const topic of topics) {
      if (!this.isUsed(topic)) added.push(topic);
      this.permanent.add(topic);
    }
    return added;
  }

  remove(owner: string, topics: string[]): { released: string[]; hasOwner: boolean } {
    const owned = this.owners.get(owner);
    if (!owned) return { released: [], hasOwner: false };
    const removed = [...new Set(topics)].filter((topic) => owned.delete(topic));
    if (!owned.size) this.owners.delete(owner);
    const released = removed.filter((topic) => !this.isUsed(topic));
    return { released, hasOwner: this.owners.has(owner) };
  }
}

/** Older servers report scoped IDs using Rust's Option debug format. */
export function hasSubscribedTopic(topics: string[], topic: string, id: string): boolean {
  return topics.includes(`${topic}.${id}`) || topics.includes(`${topic}.Some(${id})`);
}
