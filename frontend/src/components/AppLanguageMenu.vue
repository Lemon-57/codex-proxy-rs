<script setup lang="ts">
import type { IconButtonProps } from '@codex-proxy/ui'
import type { Locale } from '@/i18n/locale'
import { ZIconButton, ZMenuItem, ZPopover } from '@codex-proxy/ui'
import { Check, Languages } from '@lucide/vue'
import chineseFlag from 'circle-flags/flags/cn.svg?url&no-inline'
import englishFlag from 'circle-flags/flags/us.svg?url&no-inline'
import { computed, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLocale } from '@/composables/useLocale'

withDefaults(defineProps<{ size?: IconButtonProps['size'], variant?: IconButtonProps['variant'] }>(), {
  size: 'small',
  variant: 'text',
})

const { t } = useI18n({ useScope: 'global' })
const { locale, localeOptions, setLocale } = useLocale()
const localeFlags: Record<Locale, string> = {
  'zh-Hans': chineseFlag,
  'zh-Hant': chineseFlag,
  'en': englishFlag,
}
const choices = useTemplateRef<HTMLElement>('choices')
const label = computed(() => t('navigation.switchLanguage', {
  value: localeOptions.find(option => option.value === locale.value)?.label ?? 'English',
}))

function focusSelected() {
  choices.value?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]')?.focus()
}

function selectLocale(value: Locale, close: () => void) {
  setLocale(value)
  close()
}
</script>

<template>
  <ZPopover placement="top-end" @opened="focusSelected">
    <template #reference="{ id, open }">
      <ZIconButton :size="size" :variant="variant" :aria-label="label" :aria-expanded="open" :aria-controls="id">
        <Languages :size="size === 'small' ? 18 : 19" />
      </ZIconButton>
    </template>
    <template #default="{ close }">
      <div ref="choices" role="group" :aria-label="t('common.language')" class="grid min-w-40 gap-1 p-1.5">
        <ZMenuItem
          v-for="option in localeOptions"
          :key="option.value"
          :aria-pressed="option.value === locale"
          class="aria-pressed:bg-cp-control-item-bg-active aria-pressed:text-cp-primary-text aria-pressed:hover:bg-cp-control-item-bg-active"
          @click="selectLocale(option.value, close)"
        >
          <template #icon>
            <img :src="localeFlags[option.value]" alt="" aria-hidden="true" class="size-4.5 shrink-0">
          </template>
          <span class="flex items-center justify-between gap-3">
            <span :lang="option.value">{{ option.label }}</span>
            <Check v-if="option.value === locale" class="size-4 shrink-0" aria-hidden="true" />
          </span>
        </ZMenuItem>
      </div>
    </template>
  </ZPopover>
</template>
