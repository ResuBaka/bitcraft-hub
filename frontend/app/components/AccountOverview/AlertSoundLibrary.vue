<script setup lang="ts">
import { alertSoundLibraryKey } from "~/composables/useAlertSoundLibrary";

const library = inject(alertSoundLibraryKey)!;
const { clips } = library;
const busy = ref(false);
const errors = ref<string[]>([]);
const editing = ref<string | null>(null);
const draft = ref("");
const vSelectName = {
  mounted(input: HTMLInputElement) {
    input.focus();
    input.select();
  },
};
function startRename(id: string, name: string) {
  errors.value = [];
  draft.value = name;
  editing.value = id;
}
function cancelRename() {
  editing.value = null;
  errors.value = [];
}
function saveName(id: string) {
  try {
    library.rename(id, draft.value);
    cancelRename();
  } catch (e) {
    errors.value = [e instanceof Error ? e.message : "Could not rename the sound."];
  }
}
async function choose(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = Array.from(input.files ?? []);
  input.value = "";
  errors.value = [];
  busy.value = true;
  try {
    for (const file of files) {
      try {
        await library.upload(file);
      } catch (e) {
        errors.value.push(
          `${file.name}: ${e instanceof Error ? e.message : "Could not save sound."}`,
        );
      }
    }
  } finally {
    busy.value = false;
  }
}
function remove(id: string) {
  errors.value = [];
  try {
    library.remove(id);
  } catch {
    errors.value.push("Could not remove the saved sound.");
  }
}
</script>

<template>
  <section class="space-y-2">
    <label class="block text-sm font-medium">
      Sound library
      <input
        type="file"
        accept="audio/*"
        multiple
        :disabled="busy"
        class="mt-1 block w-full text-sm file:mr-2 file:rounded file:bg-elevated file:px-2 file:py-1"
        @change="choose"
      />
    </label>
    <p class="text-xs text-muted">
      Upload clips up to 10 seconds and 512 KB each. Reuse them across alert types and players in
      this browser.
    </p>
    <ul v-if="clips.length" class="max-h-32 overflow-auto">
      <li
        v-for="clip in clips"
        :key="clip.id"
        class="flex items-center justify-between gap-2 text-sm"
      >
        <form
          v-if="editing === clip.id"
          class="flex min-w-0 flex-1 items-center gap-1"
          @submit.prevent="saveName(clip.id)"
          @keydown.esc.stop.prevent="cancelRename"
        >
          <input
            v-model="draft"
            v-select-name
            aria-label="Sound name"
            class="min-w-0 flex-1 rounded border border-default bg-default px-1.5 py-1"
          />
          <UButton type="submit" size="xs" label="Save" />
          <UButton
            type="button"
            size="xs"
            label="Cancel"
            color="neutral"
            variant="ghost"
            @click="cancelRename"
          />
        </form>
        <template v-else>
          <span class="min-w-0 flex-1 truncate" :title="clip.name">{{ clip.name }}</span>
          <UButton
            icon="i-lucide-pencil"
            :aria-label="`Rename ${clip.name}`"
            title="Rename sound"
            size="xs"
            color="neutral"
            variant="ghost"
            :disabled="busy"
            @click="startRename(clip.id, clip.name)"
          />
          <UButton
            icon="i-lucide-trash-2"
            :aria-label="`Remove ${clip.name} from library`"
            title="Remove from library"
            size="xs"
            color="neutral"
            variant="ghost"
            :disabled="busy"
            @click="remove(clip.id)"
          />
        </template>
      </li>
    </ul>
    <p v-if="errors.length" role="alert" class="text-sm text-error">{{ errors.join(" ") }}</p>
  </section>
</template>
