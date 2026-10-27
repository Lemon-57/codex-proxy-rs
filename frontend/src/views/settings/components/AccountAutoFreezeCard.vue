<script setup lang="ts">
import { ZCard, ZCheckbox, ZForm, ZFormItem, ZIconButton, ZInput, ZPopover, ZSwitch } from '@codex-proxy/ui'

import { Activity, CircleAlert, Gauge, Snowflake, Timer } from '@lucide/vue'

import { useId } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n({ useScope: 'global' })

const enabled = defineModel<boolean>('enabled', { required: true })
const threshold = defineModel<string>('threshold', { required: true })
const windowSeconds = defineModel<string>('windowSeconds', { required: true })
const durationSeconds = defineModel<string>('durationSeconds', { required: true })
const probeEnabled = defineModel<boolean>('probeEnabled', { required: true })
const probeModel = defineModel<string>('probeModel', { required: true })
const adaptiveConcurrency = defineModel<boolean>('adaptiveConcurrency', { required: true })
const adaptiveConcurrencyHintId = useId()
const probeModelId = useId()
</script>

<template>
  <ZCard
    :title="t('settings.autoDisable.overloadProtection')"
    :description="t('settings.autoDisable.description')"
  >
    <ZForm class="max-w-6xl sm:grid-cols-2 sm:items-end">
      <ZSwitch
        v-model="enabled"
        class="col-span-full justify-self-start"
        :label="t('settings.autoDisable.enableOverloadProtection')"
      />

      <ZFormItem
        :label="t('settings.autoDisable.failureCountThreshold')"
        :description="t('settings.autoDisable.thresholdHelp')"
      >
        <ZInput
          v-model="threshold"
          :disabled="!enabled"
          :aria-label="t('settings.autoDisable.failureCountThreshold')"
          type="number"
          min="2"
          max="1000"
          step="1"
        >
          <template #prefix>
            <Activity class="size-4" />
          </template>
          <template #suffix>
            <span class="text-cp-sm">{{ t('settings.autoDisable.times') }}</span>
          </template>
        </ZInput>
      </ZFormItem>

      <ZFormItem
        :label="t('settings.autoDisable.statisticsWindow')"
        :description="t('settings.autoDisable.windowHelp')"
      >
        <ZInput
          v-model="windowSeconds"
          :disabled="!enabled"
          :aria-label="t('settings.autoDisable.statisticsWindowSeconds')"
          type="number"
          min="60"
          max="3600"
          step="1"
        >
          <template #prefix>
            <Timer class="size-4" />
          </template>
          <template #suffix>
            <span class="text-cp-sm">{{ t('settings.autoDisable.seconds') }}</span>
          </template>
        </ZInput>
      </ZFormItem>

      <ZFormItem
        :label="t('settings.autoDisable.cooldownTime')"
        :description="t('settings.autoDisable.cooldownHelp')"
      >
        <ZInput
          v-model="durationSeconds"
          :disabled="!enabled"
          :aria-label="t('settings.autoDisable.cooldownTimeInSeconds')"
          type="number"
          min="300"
          max="604800"
          step="1"
        >
          <template #prefix>
            <Snowflake class="size-4" />
          </template>
          <template #suffix>
            <span class="text-cp-sm">{{ t('settings.autoDisable.seconds') }}</span>
          </template>
        </ZInput>
      </ZFormItem>
      <ZFormItem
        :label="t('settings.autoDisable.recoveryProbeModel')"
        :description="t('settings.autoDisable.probeModelHelp')"
        :for="probeModelId"
      >
        <template #extra>
          <ZCheckbox
            v-model="probeEnabled"
            :disabled="!enabled"
            :label="t('settings.autoDisable.requireSuccessfulProbe')"
          />
        </template>
        <ZInput
          :id="probeModelId"
          v-model="probeModel"
          :disabled="!enabled || !probeEnabled"
          :aria-label="t('settings.autoDisable.recoveryProbeModel')"
          :placeholder="t('settings.autoDisable.modelPlaceholder')"
        >
          <template #prefix>
            <Gauge class="size-4" />
          </template>
        </ZInput>
      </ZFormItem>

      <div class="col-span-full flex items-center gap-1">
        <ZCheckbox
          v-model="adaptiveConcurrency"
          :disabled="!enabled"
          :label="t('settings.autoDisable.concurrencyReduction.label')"
        />
        <ZPopover class="-my-1" trigger="hover-click" placement="top-start" :hover-delay="240">
          <template #reference="{ open }">
            <ZIconButton
              :aria-label="t('settings.autoDisable.concurrencyReduction.helpTitle')"
              :title="undefined"
              class="size-6! hover:bg-transparent! active:bg-transparent!"
              :aria-expanded="open"
              :aria-describedby="open ? adaptiveConcurrencyHintId : undefined"
            >
              <CircleAlert class="size-3.5" aria-hidden="true" />
            </ZIconButton>
          </template>
          <p :id="adaptiveConcurrencyHintId" role="tooltip" class="m-0 max-w-72 px-3 py-2 text-cp-sm leading-relaxed text-cp-text-secondary">
            {{ t('settings.autoDisable.concurrencyReduction.description') }}
          </p>
        </ZPopover>
      </div>
    </ZForm>
  </ZCard>
</template>
