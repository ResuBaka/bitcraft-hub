import {deepMerge} from "@antfu/utils";
const IN_BROWSER = typeof window !== 'undefined'

export type ConfigState = {
    version: 1,
    theme: string,
}

export const DEFAULT_CONFIG: ConfigState = {
    version: 1,
    theme: 'system'
}

export const useConfigStore = defineStore('config', () => {
    const state = reactive(deepMerge({}, DEFAULT_CONFIG))

    watch(state, save)

    function load () {
        if (!IN_BROWSER) return

        const stored = localStorage.getItem('b-tool@config')
        const data = stored ? JSON.parse(stored) : {}
        let needsRefresh = data.version === state.version


        data.version = state.version
        Object.assign(state, deepMerge(state, data))
        if (needsRefresh) {
            save()
        }
    }

    function save () {
        if (!IN_BROWSER) return

        localStorage.setItem('b-tool@config', JSON.stringify(state, null, 2))
    }

    function reset () {
        if (!IN_BROWSER) return

        Object.assign(state, deepMerge({}, DEFAULT_CONFIG))

        save()
    }

    load()

    return {
        ...toRefs(state),
        load,
        save,
        reset
    }
})