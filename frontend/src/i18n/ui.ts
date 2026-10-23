import type { UiLocale } from '@codex-proxy/ui'
import type { Locale } from './locale'
import { en, zhHans, zhHant } from '@codex-proxy/ui/locale'
import { computed } from 'vue'
import { locale } from './index'

const messages: Record<Locale, UiLocale> = { en, 'zh-Hans': zhHans, 'zh-Hant': zhHant }
export const uiLocale = computed(() => messages[locale.value as Locale])
