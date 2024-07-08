import { getWebsocket, startWebsocket } from "../../../websocket";

export default defineNitroPlugin(async (nitroApp) => {
  const config = useRuntimeConfig();
  if (!config.bitcraft.websocket.enabled) {
    return;
  }

  await startWebsocket(
    config.bitcraft.url,
    config.bitcraft.auth,
    config.bitcraft.websocket,
  );
  const websocket = getWebsocket();

  Object.assign(nitroApp, { websocket });
});
