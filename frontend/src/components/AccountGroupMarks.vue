<script setup lang="ts">
import type { AccountGroupRef } from '@/api'

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  groups: AccountGroupRef[]
}>()

const { t } = useI18n({ useScope: 'global' })

const fullNames = computed(() =>
  props.groups.map(group => (group.enabled
    ? group.name
    : t('common.status.disabledName', { name: group.name }))).join('、'),
)
</script>

<template>
  <div
    v-if="groups.length > 0"
    class="flex min-w-0 flex-wrap items-center gap-1.5"
    :title="fullNames"
    :aria-label="fullNames"
  >
    <span
      v-for="group in groups"
      :key="group.id"
      class="max-w-full truncate rounded px-1.5 py-0.5 text-xs leading-4"
      :style="{
        color: group.color,
        backgroundColor: `${group.color}1a`,
      }"
      :title="group.enabled ? group.name : t('common.status.disabledName', { name: group.name })"
    >
      {{ group.enabled ? group.name : t('common.status.disabledName', { name: group.name }) }}
    </span>
  </div>
  <span v-else class="text-xs text-z-text-quaternary">—</span>
</template>
