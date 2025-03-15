import {useWebSocket} from "@vueuse/core";
import { unpack } from "msgpackr/unpack";

export const useWebsocketStore = defineStore('websocket', () => {
    const configStore = useConfigStore()
    const websocket_message_event_handler: Record<string, Map<string, (message: Record<string, any>) => void>> = {}
    const topics_currently_subscribed: Ref<string[]> = ref([])

    const readOnlyTopicsCurrentlySubscribed = computed(() => topics_currently_subscribed.value)

    const {
        public: { api },
    } = useRuntimeConfig();

    const { send, status, close, open } = useWebSocket(`${api.websocket}/websocket`, {
        onMessage: handleMessage,
        autoReconnect: {
            retries: 5,
            delay: 5000,
            onFailed() {}
        },
        immediate: false,
        onConnected: () => {
            if (import.meta.env.DEV) {
                console.log('Connected to websocket', topics_currently_subscribed.value)
            }
            sendMessage("Subscribe", {topics: topics_currently_subscribed.value})
        }
    })

    if (configStore.websocket.enabled_default) {
        open()
    }

    async function handleMessage(_ws: WebSocket, event: MessageEvent) {
        let message
        if (typeof event.data === "string") {
            if (event.data.startsWith("{")) {
                message = JSON.parse(event.data)
            } else if (event.data.startsWith("t: ")) {
                console.warn("yaml", event.data)
            } else if (event.data.startsWith("t =")) {
                console.warn("toml", event.data)
            }

        } else if (event.data instanceof Blob) {
            message = unpack(await event.data.arrayBuffer(), {
                // @ts-ignore
                int64AsType: "auto",
            });

            console.log("message", message);
        }

        if (!message) {
            console.warn("no message")
            return
        }

        const messageHandler = websocket_message_event_handler[message.t]

        if (messageHandler) {
            for (const handler of messageHandler.values()) {
                handler(message)
            }
        } else {
            if (import.meta.env.DEV) {
                console.warn(`No handler found for message type ${message.t}`)
            }
        }
    }

    function sendMessage(topic: string, message: any) {
        if (status.value !== 'OPEN') {
            return
        }

        if (message) {
            send(JSON.stringify({t: topic, c: message}))
        } else {
            send(JSON.stringify({t: topic}))
        }
    }

    function subscribe<T extends Record<string, any>>(eventType: string, topic: MaybeRefOrGetter<string | string[]>, handler: (message: T) => void, instanceId: string, lazy: boolean = false) {
        let newTopics = []
        let unwrapped = unref(topic)

        if (typeof unwrapped === 'string') {
            if (!topics_currently_subscribed.value.includes(unwrapped)) {
                newTopics.push(unwrapped)
                topics_currently_subscribed.value.push(unwrapped)
            }
        } else {
            for (const t of unwrapped) {
                if (!topics_currently_subscribed.value.includes(t)) {
                    newTopics.push(t)
                    topics_currently_subscribed.value.push(t)
                }
            }
        }

        if (!websocket_message_event_handler[eventType]) {
            websocket_message_event_handler[eventType] = new Map()
            if (!lazy && newTopics.length > 0) {
                sendMessage("Subscribe", {topics: newTopics })
            }
            websocket_message_event_handler[eventType].set(instanceId, handler)
        } else {
            if (!lazy && newTopics.length > 0) {
                sendMessage("Subscribe", {topics: newTopics })
            }
            websocket_message_event_handler[eventType].set(instanceId, handler)
        }
    }

    function subscribeTopicsOnly(topic: string | string[], lazy: boolean = false) {
        let newTopics = []
        let unwrapped = unref(topic)

        if (typeof unwrapped === 'string') {
            if (!topics_currently_subscribed.value.includes(unwrapped)) {
                newTopics.push(unwrapped)
                topics_currently_subscribed.value.push(unwrapped)
            }
        } else {
            for (const t of unwrapped) {
                if (!topics_currently_subscribed.value.includes(t)) {
                    newTopics.push(t)
                    topics_currently_subscribed.value.push(t)
                }
            }
        }

        if (!lazy && newTopics.length > 0) {
            sendMessage("Subscribe", {topics: newTopics })
        }
    }

    function unsubscribe(eventType: string,topic: MaybeRefOrGetter<string | string[]>, instanceId: string) {
        let unwrapped = unref(topic)

        let topicsToUnsubscribe: MaybeRefOrGetter<string | string[]> = []
        if (typeof unwrapped === 'string') {
            topicsToUnsubscribe.push(unwrapped)
        } else {
            topicsToUnsubscribe = unwrapped
        }

        if (websocket_message_event_handler[eventType]) {
            websocket_message_event_handler[eventType].delete(instanceId)
            if (websocket_message_event_handler[eventType].size === 0) {
                delete websocket_message_event_handler[eventType]
            }
        }

        for (const topic of topicsToUnsubscribe) {
            sendMessage("Unsubscribe", {topic: topic})
            if (topics_currently_subscribed.value.includes(topic)) {
                topics_currently_subscribed.value.splice(topics_currently_subscribed.value.indexOf(topic), 1)
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
        isConnected: computed(() => status.value === 'OPEN')
    }
})