// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { ActionType } from "./ActionType";

export type PlayerActionState = {
  auto_id: bigint;
  entity_id: bigint;
  action_type: ActionType;
  layer: any;
  last_action_result: any;
  start_time: bigint;
  duration: bigint;
  target: any;
  recipe_id: number | null;
  client_cancel: boolean;
};
