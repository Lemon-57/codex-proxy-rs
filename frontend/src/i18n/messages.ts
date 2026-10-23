import type { Locale } from './locale'
import en from './locales/en'
import zhHans from './locales/zh-Hans'
import zhHant from './locales/zh-Hant'

export type MessageSchema = typeof en

export default {
  en,
  'zh-Hans': zhHans,
  'zh-Hant': zhHant,
} satisfies Record<Locale, MessageSchema>
