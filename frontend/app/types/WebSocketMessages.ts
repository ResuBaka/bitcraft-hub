// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { ActionState } from "./ActionState";
import type { ClaimLocalState } from "./ClaimLocalState";
import type { MobileEntityState } from "./MobileEntityState";
import type { PlayerActionState } from "./PlayerActionState";
import type { PlayerState } from "./PlayerState";

export type WebSocketMessages =
  | { t: "Subscribe"; c: { topics: Array<string> } }
  | { t: "ListSubscribedTopics" }
  | { t: "SubscribedTopics"; c: Array<string> }
  | { t: "Unsubscribe"; c: { topic: string } }
  | { t: "MobileEntityState"; c: MobileEntityState }
  | {
      t: "Experience";
      c: {
        experience: bigint;
        level: bigint;
        rank: bigint;
        skill_name: string;
        user_id: bigint;
      };
    }
  | {
      t: "TotalExperience";
      c: { user_id: bigint; experience: bigint; experience_per_hour: bigint };
    }
  | {
      t: "MovedOutOfClaim";
      c: { user_id: bigint; chunk_index: bigint; claim_id: bigint };
    }
  | {
      t: "MovedIntoClaim";
      c: { user_id: bigint; chunk_index: bigint; claim_id: bigint };
    }
  | {
      t: "PlayerMovedIntoClaim";
      c: { user_id: bigint; chunk_index: bigint; claim_id: bigint };
    }
  | {
      t: "PlayerMovedOutOfClaim";
      c: { user_id: bigint; chunk_index: bigint; claim_id: bigint };
    }
  | { t: "PlayerActionState"; c: PlayerActionState }
  | { t: "PlayerActionStateChangeName"; c: [string, bigint] }
  | { t: "Level"; c: { level: bigint; user_id: bigint; skill_name: string } }
  | { t: "PlayerState"; c: PlayerState }
  | { t: "ClaimLocalState"; c: ClaimLocalState }
  | { t: "Message"; c: string }
  | { t: "ActionState"; c: ActionState };
