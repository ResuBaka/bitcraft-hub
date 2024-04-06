type Item = {
    title: string
    id: string
    building?: string
    tier: number
    tool?: string
    skill?: string
    creates: number
    items: NeededItems[]
}

type FullItem = {
    title: string
    id: string
    building?: Building
    tier: number
    tool?: Tool
    skill?: Skill
    items: FullItem[]
    amount?: number
}

enum ToolType {
    Axt = "axt",
    Pickaxe = "pickaxe",
    Hammer = "hammer",
    Hoe = "hoe",
    Knife = "knife",
    Saw = "saw",
    Quill = "quill",
    Bow = "bow",
    Rod = "rod",
}

type Building = {
    title: string
    id: string
    tier: number
    items_can_be_crafted: string[]
}

type Skill = {
    title: string
    id: string
}

type Tool = {
    title: string
    id: string
    tier: number
}

type NeededItems = {
    id: string,
    amount: number
}