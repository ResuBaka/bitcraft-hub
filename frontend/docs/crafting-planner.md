# Crafting planner

`app/utils/craftingPlan.ts` separates three kinds of state:

- **Graph:** one item record per `Item:id` / `Cargo:id`, one recipe record per recipe ID, and output/yield references between them. Ingredient lists are shared across a recipe's outputs. The graph is built only when the recipe dataset changes and treated as read-only.
- **Choices:** a map from occurrence paths to recipe/yield keys. `null` means supply the item as a base material. Paths include ancestor recipe keys so repeated materials can use different recipes. Replacing a choice removes overrides below the replaced branch; navigation clears choices.
- **Plan:** plain, serializable records for the currently selected branches only. Each occurrence stores its shortage, allocated inventory, step actions, and bottom-up total actions. There are no lazy subtree getters, historical plan caches, or parallel panel trees.

The UI reads alternative names, ingredients, and direct step costs from the shared graph. It calculates descendant quantities and totals only after a recipe is selected. Collapsed sections do not mount descendant components.

The plan browser uses Nuxt UI `UTree` with compact item rows and one item inspector, rather than recursively embedding full cards and recipe controls. The inspector is beside the tree on desktop and in a `USlideover` opened by the Details button on mobile. Rows reference the current plan nodes and use occurrence paths as keys; selecting a row does not change its recipe or recalculate the plan. Expansion/selection state contains paths only and is reconciled when the plan changes. Plans over 150 nodes use the tree virtualizer. Expand all expands only the selected plan, never graph alternatives.

Named **Recraft** and **Scrap** recipes are excluded from normal production ranks and recipe dropdowns. Each item's separate existing-item operations section indexes these recipes by their first consumed stack (the equipment), not by catalysts. It also includes special recipes producing the viewed item, so a scrap material's page can show the equipment that yields it. `app/utils/itemOperations.ts` calculates only direct input requirements and expected output yields, never a recursive plan to manufacture equipment for recycling. Operation runs are independent of the normal crafting amount and plan.

## Calculation rules

- Productive routes are ranked once over the graph. Self-recrafting does not create a productive route from nothing.
- Inventory is allocated once across the selected plan. Fully covered occurrences have no descendants or action cost.
- An explicitly selected recraft uses existing equipment as an ingredient, rather than treating it as a finished output.
- Encountering an ancestor item stops recursion and reports the missing existing input. There is no arbitrary depth cutoff.
- The shopping list contains only remaining material requirements. Extraction action estimates use expected yields.
- Filtering package ingredients must not make package-only unpack recipes appear to produce materials for free.

Memory scales with shared graph metadata, the current selected occurrences, and small choice records—not all possible recipe paths. Separate occurrences are still needed for independent choices and inventory allocation. The page still downloads the existing full recipe payload; a scoped API endpoint would be a separate optimization.

## Rarity chances in normal crafting

Each output edge stores the exact item's per-attempt probability and expected yield, including nested item-list outcomes. The selected plan propagates average retry counts into its ingredients, so earlier-tier rarities compound rather than being treated as guaranteed inputs. Fractional average demand stays fractional through the plan; only aggregated shopping totals are rounded up. Equally shallow default routes are compared using expected crafting-action costs, including prerequisite odds, without expanding alternative trees.

For fixed-size successful drops, an integer target uses `ceil(target / batch size) / probability` average attempts. Fractional demand from a stochastic parent uses the expected-yield approximation, as do variable batch sizes beyond the first drop. Inventory is offset against average demand, not a full demand distribution, and other rarities / failed outputs are not credited as substitutes or reused byproducts. These are average-cost estimates, not confidence budgets for the whole chain. Local 95% one-drop numbers in the tree do not imply 95% success for the complete plan or for multiple copies.

## Drop estimates

Chance-based operation results distinguish expected quantity from the per-run probability of at least one drop. `dropEstimates.ts` combines exclusive list outcomes and independent output stacks without enumerating full outcome combinations. It shows the geometric mean `1 / p`, cumulative chance `1 - (1 - p)^n`, and 50/90/95/99% run budgets for at least one drop. These assume independent attempts at fixed odds, with no pity system; they are not guarantees or multi-copy targets. Applying a budget updates gross operation inputs/actions. Returned equipment may be reused, so gross input uses are not a count of distinct starting items. Unsupported outcome probabilities are labelled unavailable rather than inferred from expected yield.

## Tests

```sh
bun run test
CRAFTING_TEST_API=http://localhost:8000 bun test tests/craftingPlan.integration.test.ts
```

The optional integration test exercises item `17840106` (Umbracite Plated Boots) against a real API, including repeated recipe switches. Unit tests cover normalization, unused alternatives, cycles, recrafting, inventory allocation, branch-choice cleanup, and page invalidation.
