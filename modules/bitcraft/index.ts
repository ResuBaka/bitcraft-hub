// `nuxt/kit` is a helper subpath import you can use when defining local modules
// that means you do not need to add `@nuxt/kit` to your project's dependencies
import {createResolver, defineNuxtModule, addServerHandler, addServerImportsDir} from 'nuxt/kit'

export default defineNuxtModule({
    meta: {
        name: 'hello'
    },
    setup () {
        // Add an API route
        const resolver = createResolver(import.meta.url)
        addServerImportsDir(resolver.resolve('./runtime/server'))
    }
})