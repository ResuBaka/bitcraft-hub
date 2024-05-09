import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";
import { getSkillRowsFromRows, type SkillDescRow } from "./skillDesc";

type Skills = {
  [skill_id: string]: number;
};
export interface ExpeirenceStateRow extends Entity {
  experience_stacks: Skills;
}

type ExtendedSkills = {
  [skill_name: string]: {
    experience: number;
    rank?: number;
    level: number;
  };
};
export interface ExtendedExpeirenceStateRow extends Entity {
  experience_stacks: ExtendedSkills;
}
const levelingData = [
  {
    level: 1,
    xp: 0,
  },
  {
    level: 2,
    xp: 640,
  },
  {
    level: 3,
    xp: 1330,
  },
  {
    level: 4,
    xp: 2090,
  },
  {
    level: 5,
    xp: 2920,
  },
  {
    level: 6,
    xp: 3830,
  },
  {
    level: 7,
    xp: 4820,
  },
  {
    level: 8,
    xp: 5890,
  },
  {
    level: 9,
    xp: 7070,
  },
  {
    level: 10,
    xp: 8350,
  },
  {
    level: 11,
    xp: 9740,
  },
  {
    level: 12,
    xp: 11260,
  },
  {
    level: 13,
    xp: 12920,
  },
  {
    level: 14,
    xp: 14730,
  },
  {
    level: 15,
    xp: 16710,
  },
  {
    level: 16,
    xp: 18860,
  },
  {
    level: 17,
    xp: 21210,
  },
  {
    level: 18,
    xp: 23770,
  },
  {
    level: 19,
    xp: 26560,
  },
  {
    level: 20,
    xp: 29600,
  },
  {
    level: 21,
    xp: 32920,
  },
  {
    level: 22,
    xp: 36550,
  },
  {
    level: 23,
    xp: 40490,
  },
  {
    level: 24,
    xp: 44800,
  },
  {
    level: 25,
    xp: 49490,
  },
  {
    level: 26,
    xp: 54610,
  },
  {
    level: 27,
    xp: 60200,
  },
  {
    level: 28,
    xp: 66290,
  },
  {
    level: 29,
    xp: 72930,
  },
  {
    level: 30,
    xp: 80170,
  },
  {
    level: 31,
    xp: 88060,
  },
  {
    level: 32,
    xp: 96670,
  },
  {
    level: 33,
    xp: 106060,
  },
  {
    level: 34,
    xp: 116300,
  },
  {
    level: 35,
    xp: 127470,
  },
  {
    level: 36,
    xp: 139650,
  },
  {
    level: 37,
    xp: 152930,
  },
  {
    level: 38,
    xp: 167410,
  },
  {
    level: 39,
    xp: 183200,
  },
  {
    level: 40,
    xp: 200420,
  },
  {
    level: 41,
    xp: 219200,
  },
  {
    level: 42,
    xp: 239680,
  },
  {
    level: 43,
    xp: 262020,
  },
  {
    level: 44,
    xp: 286370,
  },
  {
    level: 45,
    xp: 312930,
  },
  {
    level: 46,
    xp: 341890,
  },
  {
    level: 47,
    xp: 373480,
  },
  {
    level: 48,
    xp: 407920,
  },
  {
    level: 49,
    xp: 445480,
  },
  {
    level: 50,
    xp: 486440,
  },
  {
    level: 51,
    xp: 531110,
  },
  {
    level: 52,
    xp: 579820,
  },
  {
    level: 53,
    xp: 632940,
  },
  {
    level: 54,
    xp: 690860,
  },
  {
    level: 55,
    xp: 754030,
  },
  {
    level: 56,
    xp: 822920,
  },
  {
    level: 57,
    xp: 898040,
  },
  {
    level: 58,
    xp: 979960,
  },
  {
    level: 59,
    xp: 1069290,
  },
  {
    level: 60,
    xp: 1166710,
  },
  {
    level: 61,
    xp: 1272950,
  },
  {
    level: 62,
    xp: 1388800,
  },
  {
    level: 63,
    xp: 1515140,
  },
  {
    level: 64,
    xp: 1652910,
  },
  {
    level: 65,
    xp: 1803160,
  },
  {
    level: 66,
    xp: 1967000,
  },
  {
    level: 67,
    xp: 2145660,
  },
  {
    level: 68,
    xp: 2340500,
  },
  {
    level: 69,
    xp: 2552980,
  },
  {
    level: 70,
    xp: 2784680,
  },
  {
    level: 71,
    xp: 3037360,
  },
  {
    level: 72,
    xp: 3312900,
  },
  {
    level: 73,
    xp: 3613390,
  },
  {
    level: 74,
    xp: 3941070,
  },
  {
    level: 75,
    xp: 4298410,
  },
  {
    level: 76,
    xp: 4688090,
  },
  {
    level: 77,
    xp: 5113030,
  },
  {
    level: 78,
    xp: 5576440,
  },
  {
    level: 79,
    xp: 6081800,
  },
  {
    level: 80,
    xp: 6632890,
  },
  {
    level: 81,
    xp: 7233850,
  },
  {
    level: 82,
    xp: 7889210,
  },
  {
    level: 83,
    xp: 8603890,
  },
  {
    level: 84,
    xp: 9383250,
  },
  {
    level: 85,
    xp: 10233150,
  },
  {
    level: 86,
    xp: 11159970,
  },
  {
    level: 87,
    xp: 12170670,
  },
  {
    level: 88,
    xp: 13272850,
  },
  {
    level: 89,
    xp: 14474790,
  },
  {
    level: 90,
    xp: 15785510,
  },
  {
    level: 91,
    xp: 17214860,
  },
  {
    level: 92,
    xp: 18773580,
  },
  {
    level: 93,
    xp: 20473370,
  },
  {
    level: 94,
    xp: 22327010,
  },
  {
    level: 95,
    xp: 24348420,
  },
  {
    level: 96,
    xp: 26552780,
  },
  {
    level: 97,
    xp: 28956650,
  },
  {
    level: 98,
    xp: 31578090,
  },
  {
    level: 99,
    xp: 34436800,
  },
  {
    level: 100,
    xp: 37554230,
  },
];
export function XPToLevel(xp: number) {
  for (const data of levelingData) {
    if (xp < data.xp) {
      return data.level - 1;
    }
  }
  return 100;
}
function getExperience(rows: any[]) {
  const skills: Skills = {};
  for (const row of rows) {
    skills[row[0]] = row[1];
  }
  return skills;
}

export function getLeaderboard(
  skills: SkillDescRow[],
  expeirence: ExpeirenceStateRow[],
) {
  const leaderboard: { [key: string]: ExpeirenceStateRow[] } = {};
  for (const skill of skills) {
    const expeirenceCopy = [];

    for (const xp of expeirence) {
      expeirenceCopy.push(xp);
    }
    leaderboard[skill.name] = expeirenceCopy.sort(
      (a, b) => b.experience_stacks[skill.id] - a.experience_stacks[skill.id],
    );
  }
  return leaderboard;
}
export function getExperienceRowsFromRows(rows: any[][]) {
  const playerRows: ExpeirenceStateRow[] = [];
  for (const row of rows) {
    playerRows.push(getExperienceRowFromRow(row));
  }
  return playerRows;
}

function getExperienceRowFromRow(row: any[]) {
  const PlayerState: ExpeirenceStateRow = {
    entity_id: row[0] as unknown as number,
    experience_stacks: getExperience(row[1]),
  };
  return PlayerState;
}

export function extendExperienceRowFromRow(
  expeirence: ExpeirenceStateRow,
  leaderboard: {
    [key: string]: ExpeirenceStateRow[];
  },
  skills: SkillDescRow[],
) {
  const data: ExtendedSkills = {};
  console.log(expeirence);
  for (const skill of skills) {
    if (skill.name === "ANY") {
      continue;
    }
    data[skill.name] = {
      experience: expeirence.experience_stacks[skill.id],
      level: XPToLevel(expeirence.experience_stacks[skill.id]),
      rank:
        leaderboard[skill.name].findIndex(
          (data) => data.entity_id === expeirence.entity_id,
        ) + 1,
    };
  }
  const experienceData: ExtendedExpeirenceStateRow = {
    entity_id: expeirence.entity_id,
    experience_stacks: data,
  };
  return experienceData;
}

export async function SqlRequestAllPlayers() {
  const result = await SQLRequest<any>("SELECT * FROM ExperienceState");
  return result[0].rows;
}

export function readExperienceStateRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/ExperienceState.json`, "utf8"),
  )[0].rows;
}
