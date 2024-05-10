import {
  getExperienceRowsFromRows,
  readExperienceStateRows,
  getLeaderboard,
  XPToLevel,
  type ExpeirenceStateRow,
  type ExtendedExpeirenceStateRow,
} from "~/modules/bitcraft/gamestate/experienceState";
import { getPlayerRowsFromRows, readPlayerStateRows } from "~/modules/bitcraft/gamestate/player";
import {
  getSkillRowsFromRows,
  readSkillRows,
} from "~/modules/bitcraft/gamestate/skillDesc";

const rows = getExperienceRowsFromRows(readExperienceStateRows());
const skills = getSkillRowsFromRows(readSkillRows());
const leaderboard: Record<string, ExpeirenceStateRow[]> = getLeaderboard(skills, rows);
const playerRows = getPlayerRowsFromRows(readPlayerStateRows());

export interface ExtendedExpeirenceStateRowWithName extends ExtendedExpeirenceStateRow {
  entity_name?: string
}

export default defineEventHandler((event) => {
  const top100: Record<string, ExtendedExpeirenceStateRowWithName[]> = {};
  const playerIDs = new Set<number>();

  for (const key of Object.keys(leaderboard)) {
    const values = leaderboard[key];
    const skillIndex = skills.find((s) => s.name === key)?.id;

    if (skillIndex) {
      const sorted = values
        .sort((a, b) => b.experience_stacks[skillIndex] - a.experience_stacks[skillIndex])
        .slice(0, 100);

      top100[key] = sorted.map(s => {
        const extRow: ExtendedExpeirenceStateRowWithName = {
          entity_id: s.entity_id,
          experience_stacks: {}
        };
        for (const [key, value] of Object.entries(s.experience_stacks)) {
          if (key === 'ANY') {
            continue;
          }

          const index = parseInt(key);
          const skillName = skills.find(s => s.id === index)?.name;
          if (skillName) {
            extRow.experience_stacks[skillName] = {
              experience: value,
              level: XPToLevel(value)
            }
          }
        }
        return extRow;
      });

      for (const player of sorted) {
        playerIDs.add(player.entity_id);
      }
    }
  }

  const expTable = rows
    .map(r => ({
      entity_id: r.entity_id,
      entity_name: undefined,
      exp: Object.values(r.experience_stacks).reduce((a, b) => a + b, 0),
    } as {entity_id: number, entity_name: string | undefined, exp: number}))
    .sort((a, b) => b.exp - a.exp)
    .slice(0, 100);

  for (const entity of expTable) {
    playerIDs.add(entity.entity_id);
  }

  const lvlTable = rows
    .map(r => ({
      entity_id: r.entity_id,
      entity_name: undefined,
      lvl: Object.values(r.experience_stacks).map(a => XPToLevel(a)).reduce((a, b) => a + b, 0)
    } as {entity_id: number, entity_name: string | undefined, lvl: number}))
    .sort((a, b) => b.lvl - a.lvl)
    .slice(0, 100);


  for (const entity of lvlTable) {
    playerIDs.add(entity.entity_id);
  }

  const players = playerRows
    .filter((p) => playerIDs.has(p.entity_id))
    .map((p) => ({
      entityID: p.entity_id,
      entityName: p.username
    }));
    
  for (const entity of [...expTable, ...lvlTable]) {
    entity.entity_name = players.find((p) => p.entityID === entity.entity_id)?.entityName ?? undefined;
  }

  for (const key of Object.keys(top100)) {
    for (const entity of top100[key]) {
      entity.entity_name = players.find((p) => p.entityID === entity.entity_id)?.entityName ?? undefined;
    }
  }

  return { skills: skills.filter(s => s.id !== 1), leaderboard: top100, expTable: expTable, lvlTable: lvlTable };
});
