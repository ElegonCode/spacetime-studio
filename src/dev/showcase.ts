// Local-only visual showcase. Open http://127.0.0.1:1420/tables?showcase=1
// while running `npm run dev`; production builds never install this mock.
const showcaseEnabled = import.meta.env.DEV && new URLSearchParams(location.search).has("showcase");

if (showcaseEnabled) {
  const connectionId = "showcase-local";
  const tables = [
    {
      name: "player",
      tableType: "User",
      access: "Public",
      primaryKey: ["player_id"],
      columns: [
        { name: "player_id", type: "U64", kind: "integer" },
        { name: "username", type: "String", kind: "string" },
        { name: "level", type: "U32", kind: "integer" },
        { name: "class", type: "String", kind: "string" },
        { name: "gold", type: "U64", kind: "integer" },
        { name: "online", type: "Bool", kind: "bool" },
        { name: "last_seen", type: "Timestamp", kind: "timestamp" },
      ],
      rows: [
        ["Aeralith", 78, "Arcanist", 28450, true], ["Brindlefox", 64, "Warden", 12780, true],
        ["Caelwyn", 52, "Ranger", 36840, false], ["Duskwhisper", 89, "Vanguard", 9420, true],
        ["Eirwyn", 43, "Oracle", 51600, true], ["Fablethorn", 71, "Arcanist", 19340, false],
        ["Glimmeroak", 36, "Warden", 8750, true], ["Halcyon", 95, "Ranger", 48210, true],
        ["Isolde", 58, "Oracle", 22140, false], ["Juniper", 67, "Vanguard", 16380, true],
        ["Kestrel", 81, "Ranger", 39750, true], ["Luneth", 49, "Arcanist", 11720, false],
        ["Morrowind", 73, "Warden", 30280, true], ["Nightingale", 62, "Oracle", 45890, true],
        ["Orren", 55, "Vanguard", 7860, false], ["Peregrine", 92, "Ranger", 64120, true],
        ["Quillan", 33, "Arcanist", 9340, true], ["Riven", 86, "Vanguard", 27650, false],
        ["Sablemere", 69, "Warden", 18420, true], ["Thistle", 47, "Oracle", 33980, true],
        ["Umber", 76, "Arcanist", 14860, false], ["Vespera", 59, "Ranger", 52740, true],
        ["Wilder", 41, "Warden", 11280, true], ["Xanthe", 88, "Oracle", 41820, false],
        ["Yarrow", 63, "Vanguard", 20560, true],
      ].map(([username, level, playerClass, gold, online], index) => ({
        player_id: index + 1,
        username,
        level,
        class: playerClass,
        gold,
        online,
        last_seen: Date.parse(`2026-09-${String(28 - (index % 8)).padStart(2, "0")}T${String(9 + (index % 12)).padStart(2, "0")}:${String((index * 7) % 60).padStart(2, "0")}:00Z`) * 1000,
      })),
    },
    {
      name: "inventory_item", tableType: "User", access: "Public", primaryKey: ["item_id"],
      columns: [
        { name: "item_id", type: "U64", kind: "integer" }, { name: "owner", type: "String", kind: "string" },
        { name: "item_name", type: "String", kind: "string" }, { name: "rarity", type: "String", kind: "string" },
        { name: "quantity", type: "U32", kind: "integer" }, { name: "equipped", type: "Bool", kind: "bool" },
      ],
      rows: ["Moonfall Greatsword", "Ashenwood Staff", "Wayfinder's Compass", "Gilded Signet", "Starweave Mantle", "Emberglass Vial", "Stormcaller's Grips", "Sablewing Feather"].map((item_name, index) => ({
        item_id: index + 1, owner: ["Aeralith", "Brindlefox", "Caelwyn", "Duskwhisper", "Eirwyn"][index % 5], item_name,
        rarity: ["Legendary", "Epic", "Rare", "Uncommon", "Epic", "Common", "Rare", "Uncommon"][index], quantity: index % 3 + 1, equipped: index % 3 === 0,
      })),
    },
    {
      name: "guild", tableType: "User", access: "Public", primaryKey: ["guild_id"],
      columns: [
        { name: "guild_id", type: "U64", kind: "integer" }, { name: "name", type: "String", kind: "string" },
        { name: "realm", type: "String", kind: "string" }, { name: "member_count", type: "U32", kind: "integer" },
        { name: "rating", type: "U32", kind: "integer" },
      ],
      rows: [
        ["The Astral Guard", "Moonreach", 48, 2184], ["Emberfall Covenant", "Ashenvale", 37, 2046],
        ["Sable Company", "Frostmarch", 29, 1922], ["Dawn Chorus", "Moonreach", 52, 2265],
        ["The Verdant Oath", "Greenhollow", 24, 1804], ["Night Market", "Ashenvale", 41, 2112],
        ["Wayfarers", "Frostmarch", 18, 1746], ["Cobalt Assembly", "Moonreach", 33, 1988],
      ].map(([name, realm, member_count, rating], index) => ({ guild_id: index + 1, name, realm, member_count, rating })),
    },
    {
      name: "quest", tableType: "User", access: "Public", primaryKey: ["quest_id"],
      columns: [
        { name: "quest_id", type: "U64", kind: "integer" }, { name: "title", type: "String", kind: "string" },
        { name: "zone", type: "String", kind: "string" }, { name: "min_level", type: "U32", kind: "integer" },
        { name: "reward_gold", type: "U32", kind: "integer" }, { name: "active", type: "Bool", kind: "bool" },
      ],
      rows: [
        ["A Light in the Fog", "Moonreach", 24, 860], ["The Hollow Crown", "Ashenvale", 52, 2400],
        ["Salt on the Wind", "Frostmarch", 37, 1420], ["A Garden Reclaimed", "Greenhollow", 18, 640],
        ["The Last Cartographer", "Moonreach", 46, 1880], ["Echoes Below", "Ashenvale", 61, 3100],
        ["Letters from Home", "Frostmarch", 12, 420], ["The Starless Gate", "Greenhollow", 70, 4800],
      ].map(([title, zone, min_level, reward_gold], index) => ({ quest_id: index + 1, title, zone, min_level, reward_gold, active: index !== 5 })),
    },
    {
      name: "world_event", tableType: "User", access: "Public", primaryKey: ["event_id"],
      columns: [
        { name: "event_id", type: "U64", kind: "integer" }, { name: "event_name", type: "String", kind: "string" },
        { name: "zone", type: "String", kind: "string" }, { name: "participants", type: "U32", kind: "integer" },
        { name: "active", type: "Bool", kind: "bool" },
      ],
      rows: [
        ["The Waking Eclipse", "Moonreach", 186], ["Siege of Cinderwatch", "Ashenvale", 94],
        ["Frostfall Festival", "Frostmarch", 342], ["The Verdant Bloom", "Greenhollow", 67],
        ["Starlight Regatta", "Moonreach", 218], ["Hollowmoon Hunt", "Ashenvale", 131],
      ].map(([event_name, zone, participants], index) => ({ event_id: index + 1, event_name, zone, participants, active: index < 3 })),
    },
    {
      name: "match_result", tableType: "User", access: "Public", primaryKey: ["match_id"],
      columns: [
        { name: "match_id", type: "U64", kind: "integer" }, { name: "mode", type: "String", kind: "string" },
        { name: "winner", type: "String", kind: "string" }, { name: "duration_minutes", type: "U32", kind: "integer" },
        { name: "rating_change", type: "I32", kind: "integer" },
      ],
      rows: Array.from({ length: 16 }, (_, index) => ({
        match_id: index + 1, mode: ["Arena · 3v3", "Relic Rush", "Duel"][index % 3],
        winner: ["Aeralith", "Brindlefox", "Caelwyn", "Duskwhisper", "Eirwyn"][index % 5],
        duration_minutes: 6 + (index * 3) % 18, rating_change: [24, 16, -12, 21][index % 4],
      })),
    },
  ];

  const playerSeeds = tables[0].rows;
  for (let index = 0; index < 59; index += 1) {
    const seed = playerSeeds[index % playerSeeds.length];
    playerSeeds.push({
      ...seed,
      player_id: 26 + index,
      username: `${String(seed.username)}${Math.floor(index / playerSeeds.length) + 2}`,
      last_seen: Number(seed.last_seen) - (index + 1) * 1_800_000_000,
    });
  }

  const schema = { raw: {}, tables, reducers: ["create_player", "equip_item", "complete_quest", "start_world_event"].map((name) => ({ name, lifecycle: null, scheduledBy: null, params: [] })) };
  const connections = [{ id: connectionId, name: "Astral Realms", baseUrl: "http://127.0.0.1:3000", database: "studio-showcase", identity: null, hasToken: false, readOnly: false, createdAt: Date.now(), updatedAt: Date.now() }];

  const tauriWindow = window as Window & { __TAURI_INTERNALS__?: { invoke?: (command: string, args?: Record<string, unknown>, options?: unknown) => Promise<unknown> } };
  const internals = tauriWindow.__TAURI_INTERNALS__ ?? (tauriWindow.__TAURI_INTERNALS__ = {});
  internals.invoke = async (command, args = {}, options) => {
      if (["list_connections"].includes(command)) return connections;
      if (["test_connection"].includes(command)) return { ok: true, identity: "demo-local", databaseIdentity: "showcase", message: "Connected to the local showcase database" };
      if (["get_schema"].includes(command)) return schema;
      if (["query_table"].includes(command)) {
        const table = tables.find((item) => item.name === args.tableName);
        if (!table) throw new Error(`Unknown showcase table: ${String(args.tableName)}`);
        const page = Number(args.page ?? 0);
        const pageSize = Number(args.pageSize ?? 25);
        const start = page * pageSize;
        return { rows: table.rows.slice(start, start + pageSize), columns: table.columns, total: table.name === "player" ? 84 : table.rows.length, hasMore: start + pageSize < (table.name === "player" ? 84 : table.rows.length), page, pageSize };
      }
      if (["export_table"].includes(command)) return JSON.stringify(tables.find((item) => item.name === args.tableName)?.rows ?? []);
      if (["execute_sql"].includes(command)) return { statements: [] };
      if (["list_cli_config"].includes(command)) return { path: "local showcase", hasToken: false, servers: [] };
      if (["delete_connection", "save_connection", "create_row", "update_row", "delete_row"].includes(command)) return null;
      // The update plugin is not available in the browser showcase.
      if (command.toLowerCase().includes("plugin:updater|check")) return null;
      if (command.toLowerCase().includes("plugin:opener|open_url")) return null;
      if (command.toLowerCase().includes("plugin:process|relaunch")) return null;
      console.warn("Unhandled showcase command", command, args, options);
      return null;
  };

  localStorage.setItem("spacetime-studio.theme", "dark");
  localStorage.setItem("spacetime-studio:selected-connection", connectionId);
  localStorage.setItem("spacetime-studio:table-sidebar-width", "188");
  localStorage.setItem("spacetime-studio:table-tabs", JSON.stringify({ byConnection: { [connectionId]: { tabs: ["player", "inventory_item", "guild", "quest", "world_event"], active: "player", pinned: ["player", "guild"] } } }));

}
