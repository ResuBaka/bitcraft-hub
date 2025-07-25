import { useWebSocket } from "@vueuse/core";
import { unpack } from "msgpackr/unpack";
import type { WebSocketMessages } from "~/types/WebSocketMessages";
import type {
  RefinedMessageContentType,
  WebSocketMessageHandlers,
} from "~/types";

export const useWebsocketStore = defineStore("websocket", () => {
  const configStore = useConfigStore();
  const websocket_message_event_handler: WebSocketMessageHandlers = {};
  const topics_currently_subscribed: Ref<string[]> = ref([]);

  const readOnlyTopicsCurrentlySubscribed = computed(
    () => topics_currently_subscribed.value,
  );

  const {
    public: { api },
  } = useRuntimeConfig();

  const { send, status, close, open } = useWebSocket(
    `${api.websocket}/websocket?encoding=MessagePack`,
    {
      onMessage: handleMessage,
      autoReconnect: {
        retries: 5,
        delay: 5000,
        onFailed() {},
      },
      immediate: false,
      onConnected: () => {
        if (import.meta.env.DEV) {
          console.log(
            "Connected to websocket",
            topics_currently_subscribed.value,
          );
        }
        sendMessage("Subscribe", { topics: topics_currently_subscribed.value });
      },
    },
  );

  if (configStore.websocket.enabled_default) {
    open();
  }

  async function handleMessage(_ws: WebSocket, event: MessageEvent) {
    let message: WebSocketMessages;
    if (typeof event.data === "string") {
      if (event.data.startsWith("{")) {
        message = JSON.parse(event.data);
      } else if (event.data.startsWith("t: ")) {
        console.warn("yaml", event.data);
      } else if (event.data.startsWith("t =")) {
        console.warn("toml", event.data);
      }
    } else if (event.data instanceof Blob) {
      message = unpack(await event.data.arrayBuffer(), {
        // @ts-ignore
        int64AsType: "auto",
      });
    }

    if (!message) {
      console.warn("no message");
      return;
    }

    const eventType = message.t;
    const messageHandler = websocket_message_event_handler[eventType];

    if (messageHandler) {
      for (const handler of messageHandler.values()) {
        if ("c" in message) {
          handler(message.c as RefinedMessageContentType<typeof eventType>);
        } else {
          handler(undefined as RefinedMessageContentType<typeof eventType>);
        }
      }
    } else {
      if (import.meta.env.DEV) {
        console.warn(`No handler found for message type ${eventType}`);
      }
    }
  }

  function sendMessage(topic: string, message: any) {
    if (status.value !== "OPEN") {
      return;
    }

    if (message) {
      send(JSON.stringify({ t: topic, c: message }));
    } else {
      send(JSON.stringify({ t: topic }));
    }
  }

  function subscribe<T extends WebSocketMessages["t"]>(
    eventType: T,
    topic: MaybeRefOrGetter<string | string[]>,
    handler: (message: RefinedMessageContentType<T>) => void,
    instanceId: string,
    lazy: boolean = false,
  ) {
    let newTopics = [];
    let unwrapped = unref(topic);

    if (typeof unwrapped === "string") {
      if (!topics_currently_subscribed.value.includes(unwrapped)) {
        newTopics.push(unwrapped);
        topics_currently_subscribed.value.push(unwrapped);
      }
    } else {
      for (const t of unwrapped) {
        if (!topics_currently_subscribed.value.includes(t)) {
          newTopics.push(t);
          topics_currently_subscribed.value.push(t);
        }
      }
    }

    if (!websocket_message_event_handler[eventType]) {
      websocket_message_event_handler[eventType] = new Map();
      if (!lazy && newTopics.length > 0) {
        sendMessage("Subscribe", { topics: newTopics });
      }
      websocket_message_event_handler[eventType].set(instanceId, handler);
    } else {
      if (!lazy && newTopics.length > 0) {
        sendMessage("Subscribe", { topics: newTopics });
      }
      websocket_message_event_handler[eventType].set(instanceId, handler);
    }
  }

  function subscribeTopicsOnly(
    topic: string | string[],
    lazy: boolean = false,
  ) {
    let newTopics = [];
    let unwrapped = unref(topic);

    if (typeof unwrapped === "string") {
      if (!topics_currently_subscribed.value.includes(unwrapped)) {
        newTopics.push(unwrapped);
        topics_currently_subscribed.value.push(unwrapped);
      }
    } else {
      for (const t of unwrapped) {
        if (!topics_currently_subscribed.value.includes(t)) {
          newTopics.push(t);
          topics_currently_subscribed.value.push(t);
        }
      }
    }

    if (!lazy && newTopics.length > 0) {
      sendMessage("Subscribe", { topics: newTopics });
    }
  }

  function unsubscribe<T extends WebSocketMessages["t"]>(
    eventType: T,
    topic: MaybeRefOrGetter<string | string[]>,
    instanceId: string,
  ) {
    let unwrapped = unref(topic);

    let topicsToUnsubscribe: MaybeRefOrGetter<string | string[]> = [];
    if (typeof unwrapped === "string") {
      topicsToUnsubscribe.push(unwrapped);
    } else {
      topicsToUnsubscribe = unwrapped;
    }

    if (websocket_message_event_handler[eventType]) {
      websocket_message_event_handler[eventType].delete(instanceId);
      if (websocket_message_event_handler[eventType].size === 0) {
        delete websocket_message_event_handler[eventType];
      }
    }

    for (const topic of topicsToUnsubscribe) {
      sendMessage("Unsubscribe", { topic: topic });
      if (topics_currently_subscribed.value.includes(topic)) {
        topics_currently_subscribed.value.splice(
          topics_currently_subscribed.value.indexOf(topic),
          1,
        );
      }
    }
  }

  return {
    subscribedTopics: readOnlyTopicsCurrentlySubscribed,
    subscribeTopicsOnly,
    subscribe,
    unsubscribe,
    sendMessage,
    open,
    status,
    close,
    isConnected: computed(() => status.value === "OPEN"),
  };
});
