// Stand-in for application bootstrap: what ConfigModule would do with `load`.
import { fileURLToPath } from "node:url";
import { loadAxsConfig } from "./axs-config.js";

const load = loadAxsConfig(fileURLToPath(new URL("./config.axs", import.meta.url)));
const config = load();

console.log(`listening on ${config.server.port}`);
console.log(`max upload ${config.limits.maxUploadBytes} bytes`);
