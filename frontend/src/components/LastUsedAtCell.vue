<script setup lang="ts">
import { useTimeAgoIntl } from '@vueuse/core'
import { useI18n } from 'vue-i18n'

const props = defineProps<{ value: string | null, fullDisplay: string | null }>()
const { locale } = useI18n({ useScope: 'global' })

const display = useTimeAgoIntl(() => props.value ?? 0, {
  get locale() { return locale.value },
  relativeTimeFormatOptions: { numeric: 'auto', style: 'short' },
  insertSpace: false,
})
</script>

<template>
  <span v-if="value" :title="fullDisplay ?? undefined">{{ display }}</span>
</template>
