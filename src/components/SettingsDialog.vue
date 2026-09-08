<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import AppModal from "./ui/AppModal.vue";
import AppButton from "./ui/AppButton.vue";
import IconGlyph from "./ui/IconGlyph.vue";
import { api } from "@/lib/api";
import { messageOf, useToasts } from "@/composables/useToasts";
import { displayPath, tildePath } from "@/lib/format";
import { terminalFontStack } from "@/lib/fonts";
import type { Editor, RepoInfo, Settings } from "@/lib/types";

const props = defineProps<{
  settings: Settings;
  activeRepo: RepoInfo | null;
  editors: Editor[];
  home: string | null;
  hooksInstalled: boolean;
}>();

const emit = defineEmits<{
  close: [];
  patch: [patch: Partial<Settings>];
  repoRoot: [root: string | null];
  hooksChanged: [installed: boolean];
}>();

const { notify, fail } = useToasts();
const hookBusy = ref(false);
const hookError = ref("");

const availableFonts = ref<string[]>([]);
const customFont = ref(props.settings.terminalFont ?? "");
const showCustomFont = ref(false);

onMounted(async () => {
  availableFonts.value = await api.monospaceFonts().catch(() => []);
});

const chosenFont = computed(() => props.settings.terminalFont);
const previewStack = computed(() => terminalFontStack(chosenFont.value));
/** True when the chosen face is not one we enumerated — still allowed, since a
    font may have been installed since the app started. */
const chosenIsUnlisted = computed(
  () => !!chosenFont.value && !availableFonts.value.includes(chosenFont.value),
);

// Built in script rather than the template: Vue condenses whitespace in
// template text, which would flatten these into a single line.
const previewPrompt = "\ue0b0 ~/code/vben-admin \ue0b0 \ue725 feat/login \ue0b0 2\u2716 1\u271a";
const previewCommand = "$ npm run build \u2192 \u2714 built in 955ms";
const previewGlyphs = "\uf07b \uf015 \uf09b \uf121 \uf1d3 \ue7a8 \uf0e7 \uf00c \uf00d";

function applyCustomFont() {
  const name = customFont.value.trim();
  emit("patch", { terminalFont: name.length ? name : null });
}

const repoOverride = computed(() =>
  props.activeRepo ? (props.settings.repoRoots[props.activeRepo.path] ?? null) : null,
);

async function chooseFolder(title: string): Promise<string | null> {
  const chosen = await openDialog({ directory: true, multiple: false, title });
  return typeof chosen === "string" ? chosen : null;
}

async function pickDefaultRoot() {
  const chosen = await chooseFolder("Choose the default folder for new worktrees");
  if (chosen) emit("patch", { worktreeRoot: chosen });
}

async function pickRepoRoot() {
  const chosen = await chooseFolder(`Choose where ${props.activeRepo?.name} worktrees go`);
  if (chosen) emit("repoRoot", chosen);
}

async function toggleHooks() {
  hookBusy.value = true;
  hookError.value = "";
  try {
    if (props.hooksInstalled) {
      await api.claudeUninstallHooks();
      emit("hooksChanged", false);
      notify("Status reporting turned off", "Your Claude settings were left otherwise untouched.");
    } else {
      const path = await api.claudeInstallHooks();
      emit("hooksChanged", true);
      notify("Status reporting turned on", `Added to ${tildePath(path, props.home)}`);
    }
  } catch (e) {
    hookError.value = messageOf(e);
    fail("Could not change your Claude settings", e);
  } finally {
    hookBusy.value = false;
  }
}
</script>

<template>
  <AppModal title="Settings" wide @close="emit('close')">
    <div class="space-y-5">
      <section>
        <h3 class="mb-1.5 text-base font-medium">Where new worktrees go</h3>

        <div v-if="activeRepo" class="mb-2.5">
          <p class="mb-1 text-sm text-muted">For {{ activeRepo.name }}</p>
          <div class="flex items-center gap-2">
            <p
              data-selectable
              class="min-w-0 flex-1 truncate rounded border border-edge bg-sunken px-2.5 py-1.5 font-mono text-sm"
              :class="repoOverride ? 'text-ink' : 'text-faint'"
            >
              {{
                repoOverride
                  ? displayPath(repoOverride, home, 54)
                  : settings.worktreeRoot
                    ? `${displayPath(settings.worktreeRoot, home, 42)} (default)`
                    : "Not chosen yet"
              }}
            </p>
            <AppButton @click="pickRepoRoot">Choose…</AppButton>
            <AppButton v-if="repoOverride" variant="quiet" @click="emit('repoRoot', null)">
              Use default
            </AppButton>
          </div>
        </div>

        <p class="mb-1 text-sm text-muted">Default for every other repository</p>
        <div class="flex items-center gap-2">
          <p
            data-selectable
            class="min-w-0 flex-1 truncate rounded border border-edge bg-sunken px-2.5 py-1.5 font-mono text-sm"
            :class="settings.worktreeRoot ? 'text-ink' : 'text-faint'"
          >
            {{ settings.worktreeRoot ? displayPath(settings.worktreeRoot, home, 58) : "Not chosen yet" }}
          </p>
          <AppButton @click="pickDefaultRoot">Choose…</AppButton>
        </div>
      </section>

      <section>
        <h3 class="mb-1.5 text-base font-medium">Editor</h3>
        <p v-if="!editors.length" class="text-sm text-muted">
          No supported editor found on your PATH. Install the command-line tool for VS Code,
          Cursor, Zed or Sublime Text and reopen this window.
        </p>
        <div v-else class="flex flex-wrap gap-1.5">
          <button
            v-for="editor in editors"
            :key="editor.id"
            class="rounded border px-2.5 py-1 text-base transition-colors"
            :class="
              (settings.preferredEditor ?? editors[0]?.id) === editor.id
                ? 'border-accent bg-accent text-accent-ink'
                : 'border-edge text-muted hover:text-ink'
            "
            @click="emit('patch', { preferredEditor: editor.id })"
          >
            {{ editor.name }}
          </button>
        </div>
      </section>

      <section>
        <h3 class="mb-1.5 text-base font-medium">Terminal font</h3>
        <p class="mb-2 text-sm leading-relaxed text-muted">
          Prompts like Powerlevel10k and Starship draw their separators and icons with a Nerd
          Font. Pick one here and the preview will show the glyphs instead of blanks.
        </p>

        <div class="flex items-end gap-2">
          <label class="min-w-0 flex-1">
            <span class="mb-1 block text-sm text-muted">Typeface</span>
            <select
              class="field"
              :value="chosenFont ?? ''"
              @change="
                emit('patch', {
                  terminalFont: ($event.target as HTMLSelectElement).value || null,
                })
              "
            >
              <option value="">System default</option>
              <option v-if="chosenIsUnlisted" :value="chosenFont!">{{ chosenFont }}</option>
              <option v-for="font in availableFonts" :key="font" :value="font">{{ font }}</option>
            </select>
          </label>
          <label class="w-[86px] shrink-0">
            <span class="mb-1 block text-sm text-muted">Size</span>
            <input
              class="field"
              type="number"
              min="9"
              max="24"
              :value="settings.terminalFontSize"
              @input="
                emit('patch', {
                  terminalFontSize: Math.min(
                    24,
                    Math.max(9, Number(($event.target as HTMLInputElement).value) || 12),
                  ),
                })
              "
            />
          </label>
        </div>

        <button
          v-if="!showCustomFont"
          class="mt-1.5 text-sm text-accent hover:underline"
          @click="showCustomFont = true"
        >
          Name a font that isn't listed
        </button>
        <label v-else class="mt-2 block">
          <span class="mb-1 block text-sm text-muted">
            Font name — only fixed-width faces are listed above
          </span>
          <input
            v-model="customFont"
            class="field font-mono"
            placeholder="FiraCode Nerd Font"
            spellcheck="false"
            @keydown.enter="applyCustomFont"
            @blur="applyCustomFont"
          />
        </label>

        <div
          class="mt-2.5 space-y-0.5 overflow-x-auto rounded border border-edge bg-sunken px-3 py-2.5"
          :style="{ fontFamily: previewStack, fontSize: `${settings.terminalFontSize}px`, lineHeight: 1.5 }"
        >
          <p data-selectable class="whitespace-pre text-ink">{{ previewPrompt }}</p>
          <p data-selectable class="whitespace-pre text-ink">{{ previewCommand }}</p>
          <p data-selectable class="whitespace-pre text-muted">{{ previewGlyphs }}</p>
        </div>
        <p class="mt-1 text-xs text-faint">
          Blanks on the last line mean this face has no icon glyphs.
        </p>
      </section>

      <section>
        <h3 class="mb-1.5 text-base font-medium">Appearance</h3>
        <div class="inline-flex rounded border border-edge p-0.5">
          <button
            v-for="option in (['system', 'light', 'dark'] as const)"
            :key="option"
            class="rounded-sm px-3 py-1 text-sm capitalize transition-colors"
            :class="settings.theme === option ? 'bg-accent text-accent-ink' : 'text-muted hover:text-ink'"
            @click="emit('patch', { theme: option })"
          >
            {{ option }}
          </button>
        </div>
      </section>

      <section class="rounded border border-edge p-3">
        <div class="flex items-start justify-between gap-4">
          <div class="min-w-0">
            <h3 class="text-base font-medium">Claude Code status</h3>
            <p class="mt-0.5 text-sm leading-relaxed text-muted">
              Shows on each worktree whether Claude is working, idle, or waiting for your
              approval. This adds hooks to
              <span class="font-mono text-xs">~/.claude/settings.json</span> alongside anything
              you already have there, and removing it takes only those entries back out.
            </p>
          </div>
          <AppButton :disabled="hookBusy" @click="toggleHooks">
            {{ hooksInstalled ? "Turn off" : "Turn on" }}
          </AppButton>
        </div>
        <p v-if="hookError" data-selectable class="mt-2 flex items-start gap-2 text-sm text-danger">
          <IconGlyph name="alert" :size="14" class="mt-0.5" />
          <span>{{ hookError }}</span>
        </p>
      </section>
    </div>

    <template #actions>
      <AppButton variant="primary" @click="emit('close')">Done</AppButton>
    </template>
  </AppModal>
</template>
