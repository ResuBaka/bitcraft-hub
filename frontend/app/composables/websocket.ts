import type { OutboundWebSocketMessages } from "~/types/OutboundWebSocketMessages";
import type { RefinedMessageContentType } from "~/types";

let nextHandlerId = 0;

export function registerWebsocketMessageHandler<T extends OutboundWebSocketMessages["t"]>(
  eventType: T,
  topics: MaybeRefOrGetter<string | string[]>,
  handler: (message: RefinedMessageContentType<T>) => void,
) {
  const store = useWebsocketStore();
  // A component may register more than one handler for the same event type.
  const instanceId = `${getCurrentInstance()?.uid ?? 0}:${++nextHandlerId}`;

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
