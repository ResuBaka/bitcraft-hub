import type { WebSocketMessages } from "~/types/WebSocketMessages";

export type MessageContentType<T extends WebSocketMessages["t"]> = Extract<
  WebSocketMessages,
  { t: T }
> extends { c: infer C }
  ? C
  : never;

export type RefinedMessageContentType<T extends WebSocketMessages["t"]> =
  Extract<WebSocketMessages, { t: T }> extends { c: infer C } ? C : undefined;

export type WebSocketMessageHandlers = {
  [K in WebSocketMessages["t"]]?: Map<
    string,
    (message: RefinedMessageContentType<K>) => void
  >;
};
