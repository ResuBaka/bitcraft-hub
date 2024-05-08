import {
  getExperienceRowsFromRows,
  readExperienceStateRows,
  getLeaderboard,
  type ExpeirenceStateRow,
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
  const top100: Record<string, ExpeirenceStateRow[]> = {};
  const playerIDs = new Set<number>();

  for (const key of Object.keys(leaderboard)) {
    const values = leaderboard[key];
    const skillIndex = skills.find((s) => s.name === key)?.id;
    
    if (skillIndex) {
      const sorted = values.sort((a, b) => b.experience_stacks[skillIndex] - a.experience_stacks[skillIndex]).slice(0, 100);
      top100[key] = sorted;
      for (const player of sorted) {
        playerIDs.add(player.entity_id);
      }
    }
  }

  const players = playerRows.filter((p) => playerIDs.has(p.entity_id)).map((p) => ({
    entityID: p.entity_id,
    entityName: p.username
  }));

  return { skills: skills.filter(s => s.id !== 1), leaderboard: top100, players: players };
});
