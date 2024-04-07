export interface Root {
    professions: Profession[]
    npcs: Npc[]
    buildings: Building[]
    items: Item[]
}

export interface Profession {
    id: string
    icon: any
}

export interface Npc {
    id: string
    name: string
    recipes: Recipe[]
}

export interface Recipe {
    id: string
    name: string
    input?: Input[]
    requirement?: Requirement[]
    output?: Output[]
}

export interface Input {
    id: string
    type: string
    amount: number
}

export interface Output {
    id: string
    amount: number
}

export interface Building {
    id: string
    tier: string
    name: string
    requirement: Requirement[]
    toCraft: ToCraft[]
    recipes: Recipe[]
}

export interface Requirement {
    uuid: string
    id: string
    type: string
    level?: number
}


export interface ToCraft {
    id: string
    amount: number
}

export interface Item {
    id: string
    tier: string
    name: string
    from: From[]
    icon: any
    requirement: Requirement[]
    toCraft?: ToCraft[]
    output?: number
}

export interface From {
    id: string
    type: string
}
