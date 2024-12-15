export function registerWebsocketMessageHandler(
  eventType: string,
  topics: MaybeRefOrGetter<string | string[]>,
  handler: (message: Record<string, any>) => void,
) {
  const store = useWebsocketStore();
  const instanceId = getCurrentInstance()?.uid.toString() || "0";

  watch(
    () => toValue(topics),
    (newTopics, oldTopics) => {
      for (const index in oldTopics) {
        if (newTopics.indexOf(oldTopics[index]) === -1) {
          store.unsubscribe(eventType, oldTopics[index], instanceId);
        }
      }

      store.subscribe(eventType, topics, handler, instanceId);
    },
    {
      deep: true,
    },
  );

  store.subscribe(eventType, topics, handler, instanceId);

  onBeforeUnmount(() => {
    store.unsubscribe(eventType, topics, instanceId);
  });
}
