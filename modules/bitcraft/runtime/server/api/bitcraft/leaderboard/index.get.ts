import {
  getExperienceRowsFromRows,
  readExperienceStateRows,
  XPToLevel,
  type ExtendedExpeirenceStateRow,
} from "~/modules/bitcraft/gamestate/experienceState";
import {
  getPlayerRowsFromRows,
  readPlayerStateRows,
} from "~/modules/bitcraft/gamestate/player";
import {
  getSkillRowsFromRows,
  readSkillRows,
} from "~/modules/bitcraft/gamestate/skillDesc";

const rows = getExperienceRowsFromRows(readExperienceStateRows());
const skills = getSkillRowsFromRows(readSkillRows());
const playerRows = getPlayerRowsFromRows(readPlayerStateRows());

export interface ExtendedExpeirenceStateRowWithName
  extends ExtendedExpeirenceStateRow {
  entity_name?: string;
}

export default defineEventHandler((event) => {
  const skillLeaderBoard: Record<
    string,
    {
      player_id: number;
      player_name: string;
      experience?: number;
      level?: number;
    }[]
  > = {};

  const playerTop100Experience: {
    player_id: number;
    player_name: string;
    experience: number;
  }[] = new Array(rows.length);

  const playerTop100Level: {
    player_id: number;
    player_name: string;
    level: number;
  }[] = new Array(rows.length);

  const skillMap: Record<any, any> = {};

  for (const skill of skills) {
    skillMap[skill.id] = skill.name;
  }

  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    let totalXP = 0;
    let totalLevel = 0;
    const playerId = row.entity_id;

    for (const key in row.experience_stacks) {
      const skillIndex = parseInt(key);
      const skillName = skillMap[skillIndex];

      if (skillName === "ANY") {
        continue;
      }

      const level = XPToLevel(row.experience_stacks[key]);

      if (skillName) {
        if (!skillLeaderBoard[skillName]) {
          skillLeaderBoard[skillName] = new Array(rows.length);
        }

        skillLeaderBoard[skillName][i] = {
          player_id: playerId,
          experience: row.experience_stacks[key],
          level,
        };

        totalXP += row.experience_stacks[key];
        totalLevel += level;
      }
    }

    playerTop100Experience[i] = {
      player_id: playerId,
      experience: totalXP,
    };

    playerTop100Level[i] = {
      player_id: playerId,
      level: totalLevel,
    };
  }

  for (const key of Object.keys(skillLeaderBoard)) {
    const value = skillLeaderBoard[key];
    skillLeaderBoard[key] = value
      .sort((a, b) => b.experience - a.experience)
      .slice(0, 100);
  }

  playerTop100Experience.sort((a, b) => b.experience - a.experience);
  playerTop100Level.sort((a, b) => b.level - a.level);
  skillLeaderBoard["Experience"] = playerTop100Experience.slice(0, 100);
  skillLeaderBoard["Level"] = playerTop100Level.slice(0, 100);

  for (const key of Object.keys(skillLeaderBoard)) {
    for (const entity of skillLeaderBoard[key]) {
      entity.player_name = playerRows.find(
        (p) => p.entity_id === entity.player_id,
      )?.username;
    }
  }

  return {
    ...skillLeaderBoard,
  };
});
