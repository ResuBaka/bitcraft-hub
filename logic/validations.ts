import { z } from "zod";

export const zodProfession = z.object({
  id: z.string(),
  icon: z.string(),
});

export const zodProfessions = z.array(zodProfession);

export const zodOutput = z.object({
  id: z.string(),
  amount: z.number(),
});

export const zodOutputs = z.array(zodOutput);

export const zodInput = z.object({
  id: z.string(),
  type: z.string(),
  amount: z.number(),
});

export const zodInputs = z.array(zodInput);

export const zodRecipe = z.object({
  id: z.string(),
  name: z.string(),
  input: zodInputs,
  output: zodOutputs,
});

export const zodRecipes = z.array(zodRecipe);

export const zodToCraft = z.object({
  id: z.string(),
  amount: z.number(),
});

export const zodToCrafts = z.array(zodToCraft);

export const zodFrom = z.object({
  id: z.string(),
  type: z.string(),
});

export const zodFroms = z.array(zodFrom);

export const zodRequirement = z.object({
  id: z.string(),
  type: z.string(),
  level: z.number(),
});

export const zodRequirements = z.array(zodRequirement);

export const zodBuilding = z.object({
  id: z.string(),
  tier: z.string(),
  name: z.string(),
  requirement: zodRequirements,
  toCraft: zodToCrafts,
  recipes: zodRecipes,
});

export const zodBuildings = z.array(zodBuilding);

export const zodNpc = z.object({
  id: z.string(),
  name: z.string(),
  recipes: zodRecipes,
});

export const zodNpcs = z.array(zodNpc);

export const zodItem = z.object({
  id: z.string(),
  tier: z.string(),
  name: z.string(),
  from: zodFroms,
  icon: z.string(),
  requirement: zodRequirements,
});

export const zodItems = z.array(zodItem);
