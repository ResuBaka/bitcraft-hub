import {useWebSocket} from "@vueuse/core";

export const useWebsocketStore = defineStore('websocket', () => {
    const websocket_message_event_handler: Record<string, Map> = {}
    const topics_currently_subscribed: string[] = ref([])

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
            console.log('Connected to websocket', topics_currently_subscribed.value)
            sendMessage("Subscribe", {topics: topics_currently_subscribed.value})
        }
    })

    function handleMessage(_ws: WebSocket, event: MessageEvent) {
        const message = JSON.parse(event.data)
        const messageHandler = websocket_message_event_handler[message.t]

        if (messageHandler) {
            for (const handler of messageHandler.values()) {
                handler(message)
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

    function subscribe(eventType: string, topic: string | string[], handler: (message: any) => void, instanceId: string, lazy: boolean = false) {
        let newTopics = []

        if (typeof topic === 'string') {
            if (!topics_currently_subscribed.value.includes(topic)) {
                newTopics.push(topic)
                topics_currently_subscribed.value.push(topic)
            }
        } else {
            for (const t of topic) {
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
        }

        websocket_message_event_handler[eventType].set(instanceId, handler)
    }

    function unsubscribe(eventType: string,topic: string, handler: (message: any) => void, instanceId: string) {
        if (websocket_message_event_handler[eventType]) {
            websocket_message_event_handler[eventType].delete(instanceId)
            if (websocket_message_event_handler[eventType].size === 0) {
                delete websocket_message_event_handler[eventType]
            }
            sendMessage("Unsubscribe", {topic: topic})
            if (topics_currently_subscribed.value.includes(topic)) {
                topics_currently_subscribed.value.splice(topics_currently_subscribed.value.indexOf(topic), 1)
            }
        }
    }

    return {
        subscribedTopics: readOnlyTopicsCurrentlySubscribed,
        subscribe,
        unsubscribe,
        sendMessage,
        open,
        status,
        close,
        isConnected: computed(() => status.value === 'OPEN')
    }
})