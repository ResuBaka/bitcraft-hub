import type { WebSocketMessages } from "~/types/WebSocketMessages";

export type MessageContentType<T extends WebSocketMessages["t"]> = T extends WebSocketMessages["t"]
  ? Extract<WebSocketMessages, { t: T }> extends { c: infer C }
    ? C
    : never
  : never;

export type RefinedMessageContentType<T extends WebSocketMessages["t"]> =
  T extends WebSocketMessages["t"]
    ? Extract<WebSocketMessages, { t: T }> extends { c: infer C }
      ? C
      : undefined
    : undefined;

export type WebSocketHandlerMessage<T extends WebSocketMessages["t"]> =
  | RefinedMessageContentType<T>
  | undefined;

export type WebSocketMessageHandlers = {
  [K in WebSocketMessages["t"]]?: Map<string, (message: WebSocketHandlerMessage<K>) => void>;
};

export * from "./HouseResponse";
