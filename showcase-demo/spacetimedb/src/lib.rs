use spacetimedb::{ReducerContext, Table, Timestamp};

#[spacetimedb::table(accessor = player, public)]
#[derive(Clone)]
pub struct Player {
    #[primary_key]
    #[auto_inc]
    pub player_id: u64,
    pub username: String,
    pub level: u32,
    pub class: String,
    pub gold: u64,
    pub online: bool,
    pub last_seen: Timestamp,
}

#[spacetimedb::table(accessor = inventory_item, public)]
#[derive(Clone)]
pub struct InventoryItem {
    #[primary_key]
    #[auto_inc]
    pub item_id: u64,
    pub owner: String,
    pub item_name: String,
    pub rarity: String,
    pub quantity: u32,
    pub equipped: bool,
}

#[spacetimedb::table(accessor = guild, public)]
#[derive(Clone)]
pub struct Guild {
    #[primary_key]
    #[auto_inc]
    pub guild_id: u64,
    pub name: String,
    pub realm: String,
    pub member_count: u32,
    pub rating: u32,
}

#[spacetimedb::table(accessor = quest, public)]
#[derive(Clone)]
pub struct Quest {
    #[primary_key]
    #[auto_inc]
    pub quest_id: u64,
    pub title: String,
    pub zone: String,
    pub min_level: u32,
    pub reward_gold: u32,
    pub active: bool,
}

#[spacetimedb::table(accessor = world_event, public)]
#[derive(Clone)]
pub struct WorldEvent {
    #[primary_key]
    #[auto_inc]
    pub event_id: u64,
    pub event_name: String,
    pub zone: String,
    pub participants: u32,
    pub active: bool,
}

#[spacetimedb::table(accessor = match_result, public)]
#[derive(Clone)]
pub struct MatchResult {
    #[primary_key]
    #[auto_inc]
    pub match_id: u64,
    pub mode: String,
    pub winner: String,
    pub duration_minutes: u32,
    pub rating_change: i32,
}

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    let classes = ["Arcanist", "Warden", "Ranger", "Vanguard", "Oracle"];
    let names = [
        "Aeralith", "Brindlefox", "Caelwyn", "Duskwhisper", "Eirwyn", "Fablethorn",
        "Glimmeroak", "Halcyon", "Isolde", "Juniper", "Kestrel", "Luneth",
        "Morrowind", "Nightingale", "Orren", "Peregrine", "Quillan", "Riven",
        "Sablemere", "Thistle", "Umber", "Vespera", "Wilder", "Xanthe",
        "Yarrow", "Zephira",
    ];
    for index in 0..84u32 {
        let name = format!("{}{}", names[index as usize % names.len()], if index >= 26 { index / 26 + 1 } else { 0 });
        ctx.db.player().insert(Player {
            player_id: 0,
            username: name.clone(),
            level: 18 + (index * 7 % 63),
            class: classes[index as usize % classes.len()].to_string(),
            gold: 840 + (index as u64 * 1_347 % 42_000),
            online: index % 5 != 1,
            last_seen: ctx.timestamp - std::time::Duration::from_secs((index as u64 % 72) * 3_600),
        });
    }

    let items = [
        ("Moonfall Greatsword", "Legendary"), ("Ashenwood Staff", "Epic"),
        ("Wayfinder's Compass", "Rare"), ("Gilded Signet", "Uncommon"),
        ("Starweave Mantle", "Epic"), ("Emberglass Vial", "Common"),
        ("Stormcaller's Grips", "Rare"), ("Sablewing Feather", "Uncommon"),
    ];
    for index in 0..64u32 {
        let (item_name, rarity) = items[index as usize % items.len()];
        ctx.db.inventory_item().insert(InventoryItem {
            item_id: 0,
            owner: format!("{}{}", names[index as usize % names.len()], if index >= 26 { index / 26 + 1 } else { 0 }),
            item_name: item_name.to_string(),
            rarity: rarity.to_string(),
            quantity: 1 + index % 4,
            equipped: index % 3 == 0,
        });
    }

    for (name, realm, member_count, rating) in [
        ("The Astral Guard", "Moonreach", 48, 2_184), ("Emberfall Covenant", "Ashenvale", 37, 2_046),
        ("Sable Company", "Frostmarch", 29, 1_922), ("Dawn Chorus", "Moonreach", 52, 2_265),
        ("The Verdant Oath", "Greenhollow", 24, 1_804), ("Night Market", "Ashenvale", 41, 2_112),
        ("Wayfarers", "Frostmarch", 18, 1_746), ("Cobalt Assembly", "Moonreach", 33, 1_988),
    ] {
        ctx.db.guild().insert(Guild { guild_id: 0, name: name.into(), realm: realm.into(), member_count, rating });
    }

    for (title, zone, min_level, reward_gold) in [
        ("A Light in the Fog", "Moonreach", 24, 860), ("The Hollow Crown", "Ashenvale", 52, 2_400),
        ("Salt on the Wind", "Frostmarch", 37, 1_420), ("A Garden Reclaimed", "Greenhollow", 18, 640),
        ("The Last Cartographer", "Moonreach", 46, 1_880), ("Echoes Below", "Ashenvale", 61, 3_100),
        ("Letters from Home", "Frostmarch", 12, 420), ("The Starless Gate", "Greenhollow", 70, 4_800),
    ] {
        ctx.db.quest().insert(Quest { quest_id: 0, title: title.into(), zone: zone.into(), min_level, reward_gold, active: true });
    }

    for (event_name, zone, participants) in [
        ("The Waking Eclipse", "Moonreach", 186), ("Siege of Cinderwatch", "Ashenvale", 94),
        ("Frostfall Festival", "Frostmarch", 342), ("The Verdant Bloom", "Greenhollow", 67),
        ("Starlight Regatta", "Moonreach", 218), ("Hollowmoon Hunt", "Ashenvale", 131),
    ] {
        ctx.db.world_event().insert(WorldEvent { event_id: 0, event_name: event_name.into(), zone: zone.into(), participants, active: true });
    }

    for index in 0..28u32 {
        let name = format!("{}{}", names[index as usize % names.len()], if index >= 26 { index / 26 + 1 } else { 0 });
        ctx.db.match_result().insert(MatchResult {
            match_id: 0,
            mode: ["Arena · 3v3", "Relic Rush", "Duel"][index as usize % 3].into(),
            winner: name,
            duration_minutes: 6 + index % 18,
            rating_change: if index % 3 == 0 { 24 } else if index % 3 == 1 { 16 } else { -12 },
        });
    }
}

#[spacetimedb::reducer]
pub fn refresh_demo(_ctx: &ReducerContext) {}
