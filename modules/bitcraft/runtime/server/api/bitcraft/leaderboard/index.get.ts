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

export default defineEventHandler((event) => {
  const top100: Record<string, ExtendedExpeirenceStateRow[]> = {};
  const playerIDs = new Set<number>();

  for (const key of Object.keys(leaderboard)) {
    const values = leaderboard[key];
    const skillIndex = skills.find((s) => s.name === key)?.id;

    if (skillIndex) {
      const sorted = values
        .sort((a, b) => b.experience_stacks[skillIndex] - a.experience_stacks[skillIndex])
        .slice(0, 100);

      top100[key] = sorted.map(s => {
        const extRow: ExtendedExpeirenceStateRow = {
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
      exp: Object.values(r.experience_stacks).reduce((a, b) => a + b, 0)
    }))
    .sort((a, b) => b.exp - a.exp)
    .slice(0, 100);

  for (const entity of expTable) {
    playerIDs.add(entity.entity_id);
  }

  const lvlTable = rows
    .map(r => ({
      entity_id: r.entity_id,
      lvl: Object.values(r.experience_stacks).map(a => XPToLevel(a)).reduce((a, b) => a + b, 0)
    }))
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

  return { skills: skills.filter(s => s.id !== 1), leaderboard: top100, players: players, expTable: expTable, lvlTable: lvlTable };
});
