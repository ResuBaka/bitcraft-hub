import type { WebSocketMessages } from "~/types/WebSocketMessages";
import type { RefinedMessageContentType } from "~/types";

export function registerWebsocketMessageHandler<
  T extends WebSocketMessages["t"],
>(
  eventType: T,
  topics: MaybeRefOrGetter<string | string[]>,
  handler: (message: RefinedMessageContentType<T>) => void,
) {
  const store = useWebsocketStore();
  const instanceId = getCurrentInstance()?.uid.toString() || "0";

  watch(
    () => toValue(topics),
    (newTopics, oldTopics) => {
      const oldTopic = Array.isArray(oldTopics) ? oldTopics : [oldTopics];
      const newTopic = Array.isArray(newTopics) ? newTopics : [newTopics];

      let difference = oldTopic.filter((x) => !newTopic.includes(x));

      for (let index = 0; index < difference.length; index++) {
        store.unsubscribe(eventType, difference[index], instanceId);
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
