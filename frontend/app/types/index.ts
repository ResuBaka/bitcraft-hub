import type { OutboundWebSocketMessages } from "~/types/OutboundWebSocketMessages";

export type MessageContentType<T extends OutboundWebSocketMessages["t"]> =
  T extends OutboundWebSocketMessages["t"]
    ? Extract<OutboundWebSocketMessages, { t: T }> extends { c: infer C }
      ? C
      : never
    : never;

export type RefinedMessageContentType<T extends OutboundWebSocketMessages["t"]> =
  T extends OutboundWebSocketMessages["t"]
    ? Extract<OutboundWebSocketMessages, { t: T }> extends { c: infer C }
      ? C
      : undefined
    : undefined;

export type WebSocketHandlerMessage<T extends OutboundWebSocketMessages["t"]> =
  | RefinedMessageContentType<T>
  | undefined;

export type WebSocketMessageHandlers = {
  [K in OutboundWebSocketMessages["t"]]?: Map<
    string,
    (message: WebSocketHandlerMessage<K>) => void
  >;
};

export * from "./HouseResponse";
