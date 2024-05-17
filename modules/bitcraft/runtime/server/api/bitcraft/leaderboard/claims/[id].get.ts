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
import {
  getClaimDescriptionRowsFromRows,
  readClaimRows,
} from "~/modules/bitcraft/gamestate/claimDescription";

const rows = getExperienceRowsFromRows(readExperienceStateRows());
const skills = getSkillRowsFromRows(readSkillRows());
const playerRows = getPlayerRowsFromRows(readPlayerStateRows());
const claimsRows = getClaimDescriptionRowsFromRows(readClaimRows());

export interface ExtendedExpeirenceStateRowWithName
  extends ExtendedExpeirenceStateRow {
  entity_name?: string;
}

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const claims = claimsRows.find((claims) => claims.entity_id == parseInt(id));

  if (!claims) {
    throw createError({
      statusCode: 404,
      statusMessage: "Claim was not found",
    });
  }

  const players = claims.members.map((member) => member.entity_id);

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
  }[] = new Array(players.length);

  const playerTop100Level: {
    player_id: number;
    player_name: string;
    level: number;
  }[] = new Array(players.length);

  const skillMap: Record<any, any> = {};

  for (const skill of skills) {
    skillMap[skill.id] = skill.name;
  }

  const experienceRowsToCheck = rows.filter((row) => {
    for (const player of players) {
      if (row.entity_id === player) {
        return true;
      }
    }

    return false;
  });

  for (let i = 0; i < experienceRowsToCheck.length; i++) {
    const row = experienceRowsToCheck[i];
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
          skillLeaderBoard[skillName] = new Array(claims.members.length);
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
    skillLeaderBoard[key] = value.sort((a, b) => b.experience - a.experience);
  }

  playerTop100Experience.sort((a, b) => b.experience - a.experience);
  playerTop100Level.sort((a, b) => b.level - a.level);
  skillLeaderBoard["Experience"] = playerTop100Experience;
  skillLeaderBoard["Level"] = playerTop100Level;

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
