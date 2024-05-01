import WebSocket from "ws";
import {
  diffItemsInInventorys,
  getInventoryRowFromRow,
  replaceInventoryItemIdWithItem,
} from "~/modules/bitcraft/gamestate/inventory";
import { ExpendedRefrence } from "~/modules/bitcraft/gamestate/item";
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
let counter = 0;
export default defineNitroPlugin(async (nitroApp) => {
  const usersByIdenity = getUserMapFromRows(await SqlRequestAllUsers());
  const PlayerByEntityId = getPlayerEntityIdMapFromRows(
    await SqlRequestAllPlayers(),
  );
  let websocket: WebSocket | null = null;
  try {
    websocket = new WebSocket(
      "wss://alpha-playtest-1.spacetimedb.com/database/subscribe/bitcraft-alpha",
      "v1.text.spacetimedb",
      {
        headers: {
          Authorization:
            "Basic dG9rZW46ZXlKMGVYQWlPaUpLVjFRaUxDSmhiR2NpT2lKRlV6STFOaUo5LmV5Sm9aWGhmYVdSbGJuUnBkSGtpT2lJeFpXUXlZelJsWVRsbVlUVmtaVFZqTURKaVltVTBNMkV6TldFd05XSTVabVZsTlRVek9ESmhPR0l5WldZd04yRTVaVEk0TnprMU1qUXlPR1ZqTVdFNUlpd2lhV0YwSWpveE56RXpOVFkwTkRZekxDSmxlSEFpT201MWJHeDkua2cyUHBfQ0t5OE1hcTJBT0xDeW0tckRneENkaS01MUZZV05JZ0VhQjJhMnB0YVNTRk11cGdUOXFOVWp3NVlfYkxHOERGcV8yRkxTLWhBRmVmbEU2SFE=",
          "Sec-WebSocket-Protocol": "v1.text.spacetimedb",
          "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
        },
        protocolVersion: 13,
        maxPayload: 1024 * 1024 * 1024,
      },
    );

    websocket.on("error", (error) => {
      console.error("Error with bitcraft websocket connection :: ", error);
    });
    websocket.on("open", async () => {
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
      //console.log(JSON.stringify(jsonData, null, 2))
      if (jsonData?.TransactionUpdate !== undefined) {
        const callerIdentiy: string =
          jsonData.TransactionUpdate.event.caller_identity;
        const table_updates =
          jsonData?.TransactionUpdate?.subscription_update?.table_updates[0]
            ?.table_row_operations;
        //console.log(table_updates[0].row)
        var orderedTables: { [key: string]: { delete: any; insert: any } } =
          table_updates.reduce((x: any, y: any) => {
            if (x[y.row[1]] === undefined) {
              x[y.row[1]] = {};
            }
            x[y.row[1]][y.op] = y;

            return x;
          }, {});
        for (const table of Object.values(orderedTables)) {
          //console.log(table)
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
              created: replaceInventoryItemIdWithItem(inventory),
            };
          } else if (table?.insert?.row === undefined) {
            const inventory = getInventoryRowFromRow(table?.delete?.row);
            info = {
              inventory_id: inventory.entity_id,
              timestamp: jsonData?.TransactionUpdate?.event?.timestamp,
              identity: callerIdentiy,
              deleted: replaceInventoryItemIdWithItem(inventory),
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
    websocket.on("close", (a) => {
      console.log("Disconnected");
      console.error(a);
      console.log("Disconnected");
    });

    Object.assign(nitroApp, { websocket });
  } catch (error) {
    console.error("Error with bitcraft websocket connection :: ", error);
  }
});

async function createFileIfNotExist(path: string) {
  try {
    await readFile(path);
  } catch {
    await writeFile(path, "");
  }
}
