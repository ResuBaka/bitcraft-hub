// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { CargoDesc } from "./CargoDesc";
import type { ItemDesc } from "./ItemDesc";

export type ItemCargo =
  | ({ type: "Item" } & ItemDesc)
  | ({ type: "Cargo" } & CargoDesc);
