import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";
import { type SkillDescRow } from "./skillDesc";

export type Skills = {
  [skill_id: string]: number;
};

export interface ExpeirenceStateRow extends Entity {
  experience_stacks: Skills;
}

export type ExtendedSkills = {
  [skill_name: string]: {
    experience: number;
    rank?: number;
    level: number;
  };
};

export interface ExtendedExpeirenceStateRow extends Entity {
  experience_stacks: ExtendedSkills;
}

export type LevelList = {
  [level: number]: number;
};

export const levelingData: LevelList = {
  1: 0,
  2: 640,
  3: 1330,
  4: 2090,
  5: 2920,
  6: 3830,
  7: 4820,
  8: 5890,
  9: 7070,
  10: 8350,
  11: 9740,
  12: 11260,
  13: 12920,
  14: 14730,
  15: 16710,
  16: 18860,
  17: 21210,
  18: 23770,
  19: 26560,
  20: 29600,
  21: 32920,
  22: 36550,
  23: 40490,
  24: 44800,
  25: 49490,
  26: 54610,
  27: 60200,
  28: 66290,
  29: 72930,
  30: 80170,
  31: 88060,
  32: 96670,
  33: 106060,
  34: 116300,
  35: 127470,
  36: 139650,
  37: 152930,
  38: 167410,
  39: 183200,
  40: 200420,
  41: 219200,
  42: 239680,
  43: 262020,
  44: 286370,
  45: 312930,
  46: 341890,
  47: 373480,
  48: 407920,
  49: 445480,
  50: 486440,
  51: 531110,
  52: 579820,
  53: 632940,
  54: 690860,
  55: 754030,
  56: 822920,
  57: 898040,
  58: 979960,
  59: 1069290,
  60: 1166710,
  61: 1272950,
  62: 1388800,
  63: 1515140,
  64: 1652910,
  65: 1803160,
  66: 1967000,
  67: 2145660,
  68: 2340500,
  69: 2552980,
  70: 2784680,
  71: 3037360,
  72: 3312900,
  73: 3613390,
  74: 3941070,
  75: 4298410,
  76: 4688090,
  77: 5113030,
  78: 5576440,
  79: 6081800,
  80: 6632890,
  81: 7233850,
  82: 7889210,
  83: 8603890,
  84: 9383250,
  85: 10233150,
  86: 11159970,
  87: 12170670,
  88: 13272850,
  89: 14474790,
  90: 15785510,
  91: 17214860,
  92: 18773580,
  93: 20473370,
  94: 22327010,
  95: 24348420,
  96: 26552780,
  97: 28956650,
  98: 31578090,
  99: 34436800,
  100: 7554230,
};

export function XPToLevel(xp: number): number {
  for (const level in Object.keys(levelingData)) {
    const levelXp = levelingData[parseInt(level) + 1];

    if (levelXp === undefined) {
      return parseInt(level);
    }

    if (xp < levelXp) {
      return parseInt(level);
    }
  }

  return 100;
}

function getExperience(rows: any[]): Skills {
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

export function getExperienceRowsFromRows(rows: any[][]): ExpeirenceStateRow[] {
  const playerRows: ExpeirenceStateRow[] = [];

  for (const row of rows) {
    playerRows.push(getExperienceRowFromRow(row));
  }

  return playerRows;
}

function getExperienceRowFromRow(row: any[]): ExpeirenceStateRow {
  return {
    entity_id: row[0] as unknown as number,
    experience_stacks: getExperience(row[1]),
  };
}

export function extendExperienceRowFromRow(
  expeirence: ExpeirenceStateRow,
  leaderboard: {
    [key: string]: ExpeirenceStateRow[];
  },
  skills: SkillDescRow[],
): ExtendedExpeirenceStateRow {
  const data: ExtendedSkills = {};

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

  return {
    entity_id: expeirence.entity_id,
    experience_stacks: data,
  };
}

export async function SqlRequestAllPlayers(): Promise<any> {
  const result = await SQLRequest<any>("SELECT * FROM ExperienceState");
  return result[0].rows;
}

export function readExperienceStateRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/ExperienceState.json`, "utf8"),
  )[0].rows;
}
