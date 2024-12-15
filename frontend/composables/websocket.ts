export function registerWebsocketMessageHandler(
  eventType: string,
  topics: string | string[],
  handler: (message: any) => void,
) {
  const store = useWebsocketStore();
  const instanceId = getCurrentInstance()?.uid.toString() || "0";
  store.subscribe(eventType, topics, handler, instanceId);

  onBeforeUnmount(() => {
    store.unsubscribe(eventType, topics, handler, instanceId);
  });
}
