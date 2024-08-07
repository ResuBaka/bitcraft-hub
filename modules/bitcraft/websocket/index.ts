import WebSocket from "ws";
import {
  diffItemsInInventorys,
  getInventoryRowFromRow,
  replaceInventoryItemIdWithItem,
} from "~/modules/bitcraft/gamestate/inventory";
import {
  getItemRowsFromRows,
  readItemRows,
  type ExpendedRefrence,
} from "~/modules/bitcraft/gamestate/item";
import {
  SqlRequestAllPlayers,
  getPlayerEntityIdMapFromRows,
} from "~/modules/bitcraft/gamestate/player";
import {
  SqlRequestAllUsers,
  getUserMapFromRows,
} from "~/modules/bitcraft/gamestate/userState";
import { readFile, writeFile, appendFile } from "node:fs/promises";

const storagePath = `${process.cwd()}/storage`;
const items = getItemRowsFromRows();
let websocket: WebSocket | null = null;

let counter = 0;
export async function startWebsocket(
  url: string,
  auth: { username: string; password: string },
  websocketConfig: { enabled: boolean; url: string },
  restartCounter = 0,
) {
  const usersByIdenity = getUserMapFromRows(await SqlRequestAllUsers());
  const PlayerByEntityId = getPlayerEntityIdMapFromRows(
    await SqlRequestAllPlayers(),
  );
  try {
    console.log("Connecting to bitcraft websocket");
    websocket = new WebSocket(websocketConfig.url, "v1.text.spacetimedb", {
      headers: {
        Authorization: `Basic ${btoa(`${auth.username}:${auth.password}`)}`,
        "Sec-WebSocket-Protocol": "v1.text.spacetimedb",
        "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
      },
      protocolVersion: 13,
      maxPayload: 1024 * 1024 * 1024,
    });

    websocket.on("error", (error: any) => {
      console.error("Error with bitcraft websocket connection :: ", error);
    });

    websocket.on("open", async () => {
      console.log("Connected to bitcraft websocket");
      restartCounter = 0;
      websocket.send(
        JSON.stringify({
          subscribe: {
            query_strings: ["SELECT * FROM InventoryState"],
          },
        }),
      );
    });
    websocket.on("message", async (data: any) => {
      const jsonData = JSON.parse(data.toString());
      if (jsonData?.TransactionUpdate !== undefined) {
        const callerIdentiy: string =
          jsonData.TransactionUpdate.event.caller_identity;
        const table_updates =
          jsonData?.TransactionUpdate?.subscription_update?.table_updates[0]
            ?.table_row_operations;
        var orderedTables: { [key: string]: { delete: any; insert: any } } =
          table_updates.reduce((x: any, y: any) => {
            if (x[y.row[1]] === undefined) {
              x[y.row[1]] = {};
            }
            x[y.row[1]][y.op] = y;

            return x;
          }, {});
        for (const table of Object.values(orderedTables)) {
          let info: {
            inventory_id: number;
            identity: string;
            playerName?: string;
            playerEntityId?: number;
            timestamp: number;
            created?: any;
            deleted?: any;
            diff?: {
              [key: number]: {
                old: ExpendedRefrence | undefined;
                new: ExpendedRefrence | undefined;
              };
            };
          };
          if (table?.delete?.row === undefined) {
            const inventory = getInventoryRowFromRow(table?.insert?.row);
            info = {
              inventory_id: inventory.entity_id,
              timestamp: jsonData?.TransactionUpdate?.event?.timestamp,
              identity: callerIdentiy,
              created: replaceInventoryItemIdWithItem(inventory, items),
            };
          } else if (table?.insert?.row === undefined) {
            const inventory = getInventoryRowFromRow(table?.delete?.row);
            info = {
              inventory_id: inventory.entity_id,
              timestamp: jsonData?.TransactionUpdate?.event?.timestamp,
              identity: callerIdentiy,
              deleted: replaceInventoryItemIdWithItem(inventory, items),
            };
          } else {
            const oldInventory = getInventoryRowFromRow(table?.delete?.row);
            const newInventory = getInventoryRowFromRow(table?.insert?.row);
            info = {
              inventory_id: oldInventory.entity_id,
              timestamp: jsonData?.TransactionUpdate?.event?.timestamp,
              identity: callerIdentiy,
              diff: diffItemsInInventorys(oldInventory, newInventory),
            };
          }
          const user = usersByIdenity.get(callerIdentiy);
          if (user !== undefined) {
            info.playerEntityId = user;
            info.playerName = PlayerByEntityId.get(user)?.username;
          }

          await createFileIfNotExist(
            `${storagePath}/Inventory/${info.inventory_id}.json`,
          );

          if (import.meta.dev) {
            await createFileIfNotExist(
              `${storagePath}/Inventory/${info.inventory_id}_latest.json`,
            );
            await writeFile(
              `${storagePath}/Inventory/${info.inventory_id}_latest.json`,
              JSON.stringify(info, null, 3),
            );
          }
          await appendFile(
            `${storagePath}/Inventory/${info.inventory_id}.json`,
            `${JSON.stringify(info)}\n`,
          );
        }
      }
    });

    websocket.on("close", (close: any) => {
      console.log("Disconnected");
      console.error(close);
      console.log("Disconnected");

      if (restartCounter > 30) {
        console.error("Too many restarts, exiting");
      }

      setTimeout(
        () => {
          startWebsocket(url, auth, websocketConfig, restartCounter + 1);
        },
        1000 * 60 * (restartCounter === 0 ? 1 : restartCounter),
      );
    });
  } catch (error) {
    console.error("Error with bitcraft websocket connection :: ", error);
  }
}

export function getWebsocket() {
  return websocket;
}

async function createFileIfNotExist(path: string) {
  try {
    await readFile(path);
  } catch {
    await writeFile(path, "");
  }
}
