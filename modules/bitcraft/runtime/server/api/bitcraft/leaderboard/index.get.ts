import {
  getExperienceRowsFromRows,
  readExperienceStateRows,
  extendExperienceRowFromRow,
  getLeaderboard,
  type ExpeirenceStateRow,
} from "~/modules/bitcraft/gamestate/experienceState";
import {
  getSkillRowsFromRows,
  readSkillRows,
} from "~/modules/bitcraft/gamestate/skillDesc";

const rows = getExperienceRowsFromRows(readExperienceStateRows());
const skills = getSkillRowsFromRows(readSkillRows());
const leaderboard: Record<string, ExpeirenceStateRow[]> = getLeaderboard(skills, rows);

export default defineEventHandler((event) => {
  const { neededInCrafting, producedInCrafting, neededToCraft } = getQuery(event);
  console.log("####VALUES####")
  const top100: Record<string, ExpeirenceStateRow[]> = {};
  
  for (const key of Object.keys(leaderboard)) {
    const values = leaderboard[key]
    const skillIndex = skills.find(s => s.name === key)?.id;
    // console.log("####VALUES####")
    // console.log(skills.find(s => s.name === key)?.name, values.slice(0,4))
    
    

    if(skillIndex){
        // const index = skillIndex.toString();
        // const sorted = values.sort((a, b) => b.experience_stacks[skillIndex] - a.experience_stacks[skillIndex])
        // console.log("####SORT####", skillIndex)
        // console.log(skills.find(s => s.name === key)?.name, sorted.slice(0,4))

        top100[key] = values.sort((a, b) => b.experience_stacks[skillIndex] - a.experience_stacks[skillIndex]).slice(0, 100)
    }

    //top100[key] = values.sort((a, b) => b.experience_stacks['2'] - a.experience_stacks[2]).slice(0, 100);

  }




//   console.group();
//   console.log(rows);
//   console.log(skills);
//   console.log(leaderboard);
//   console.groupEnd();
  return {skills: skills, leaderboard: top100};

});
