<script setup lang="ts">
import type { ApiKeyFormValue } from '../composables/useApiKeyEditor'

import type { AccountGroupRef } from '@/api'
import { ZButton, ZDialog, ZForm, ZFormItem, ZInput } from '@codex-proxy/ui'
import { DollarSign, KeyRound } from '@lucide/vue'

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import AccountGroupCheckboxGrid from '@/components/AccountGroupCheckboxGrid.vue'
import ProviderRequestProfilesEditor from '@/components/client-profile/ProviderRequestProfilesEditor.vue'

const props = defineProps<{
  groups: AccountGroupRef[]
  groupLoading: boolean
  editing: boolean
  saving: boolean
}>()

const emit = defineEmits<{
  save: []
  afterLeave: []
}>()

const { t } = useI18n({ useScope: 'global' })

const open = defineModel<boolean>({ default: false })
const form = defineModel<ApiKeyFormValue>('form', { required: true })
const title = computed(() => props.editing ? t('keys.actions.editKey') : t('keys.filters.createApiKey'))

function updateForm<Key extends keyof ApiKeyFormValue>(key: Key, value: ApiKeyFormValue[Key]) {
  form.value = { ...form.value, [key]: value }
}
</script>

<template>
  <ZDialog
    v-model="open"
    :title="title"
    :description="t('keys.editor.description')"
    type="info"
    width="48rem"
    :show-close="!saving" :close-on-click-modal="!saving" :close-on-press-escape="!saving"
    @closed="emit('afterLeave')"
  >
    <template #icon>
      <KeyRound class="text-cp-text" :size="20" aria-hidden="true" />
    </template>

    <ZForm class="grid gap-6">
      <ZFormItem :label="t('common.fields.name')" required>
        <ZInput
          :model-value="form.name" :aria-label="t('common.fields.name')"
          :placeholder="t('keys.editor.namePlaceholder')"
          :disabled="saving"
          @update:model-value="updateForm('name', $event)"
        />
      </ZFormItem>

      <div class="grid gap-6" :class="{ 'sm:grid-cols-2': !editing }">
        <ZFormItem :label="t('keys.editor.labelOptional')">
          <ZInput
            :model-value="form.label" :aria-label="t('keys.editor.labelOptional')"
            :placeholder="t('keys.editor.forExampleBackendService')"
            :disabled="saving"
            @update:model-value="updateForm('label', $event)"
          />
        </ZFormItem>

        <ZFormItem
          v-if="!editing"
          :label="t('keys.editor.customKeyOptional')"
        >
          <ZInput
            :model-value="form.customKey" type="password"
            autocomplete="new-password"
            :spellcheck="false"
            :aria-label="t('keys.editor.customKeyOptional')"
            :placeholder="t('keys.editor.keyPlaceholder')"
            :disabled="saving"
            @update:model-value="updateForm('customKey', $event)"
          />
        </ZFormItem>
      </div>

      <ZFormItem :label="t('groups.group')">
        <AccountGroupCheckboxGrid
          :model-value="form.groupIds" :groups="groups"
          :loading="groupLoading"
          :disabled="saving"
          @update:model-value="updateForm('groupIds', $event)"
        />
      </ZFormItem>

      <ZFormItem :label="t('keys.editor.upstreamIdentity')">
        <ProviderRequestProfilesEditor
          :model-value="form.providerRequestProfileOverrides" allow-inherit
          :active="open"
          :disabled="saving"
          @update:model-value="updateForm('providerRequestProfileOverrides', $event)"
        />
      </ZFormItem>

      <div class="grid gap-6 sm:grid-cols-2">
        <ZFormItem :label="t('keys.editor.dailyLimit')">
          <ZInput
            :model-value="form.dailyLimitUsd" type="number"
            min="0"
            step="any"
            :aria-label="t('keys.editor.dailyLimitUsd')"
            :placeholder="t('common.values.unlimited')"
            :disabled="saving"
            @update:model-value="updateForm('dailyLimitUsd', $event)"
          >
            <template #prefix>
              <DollarSign class="size-4" aria-hidden="true" />
            </template>
          </ZInput>
        </ZFormItem>
        <ZFormItem :label="t('keys.editor.weeklyLimit')">
          <ZInput
            :model-value="form.weeklyLimitUsd" type="number"
            min="0"
            step="any"
            :aria-label="t('keys.editor.weeklyLimitUsd')"
            :placeholder="t('common.values.unlimited')"
            :disabled="saving"
            @update:model-value="updateForm('weeklyLimitUsd', $event)"
          >
            <template #prefix>
              <DollarSign class="size-4" aria-hidden="true" />
            </template>
          </ZInput>
        </ZFormItem>
      </div>

      <div class="grid gap-6 sm:grid-cols-2">
        <ZFormItem :label="t('keys.editor.maximumConcurrency')">
          <ZInput
            :model-value="form.maxConcurrency" type="number"
            :aria-label="t('keys.editor.maximumConcurrency')"
            min="0"
            step="1"
            :placeholder="t('common.values.unlimited')"
            :disabled="saving"
            @update:model-value="updateForm('maxConcurrency', $event)"
          />
        </ZFormItem>
        <ZFormItem :label="t('keys.editor.requestsPerMinuteRpm')">
          <ZInput
            :model-value="form.requestsPerMinute" type="number"
            :aria-label="t('keys.editor.requestsPerMinuteRpm')"
            min="0"
            step="1"
            :placeholder="t('common.values.unlimited')"
            :disabled="saving"
            @update:model-value="updateForm('requestsPerMinute', $event)"
          />
        </ZFormItem>
      </div>
    </ZForm>

    <template #footer>
      <ZButton :disabled="saving" @click="open = false">
        {{ t('common.actions.cancel') }}
      </ZButton>
      <ZButton
        type="primary"
        :loading="saving"
        :disabled="!form.name.trim()"
        @click="emit('save')"
      >
        {{ editing ? t('keys.editor.saveChanges') : t('common.actions.create') }}
      </ZButton>
    </template>
  </ZDialog>
</template>
