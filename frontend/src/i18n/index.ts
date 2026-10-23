import { createI18n } from 'vue-i18n'
import { readInitialLocale } from './locale'
import messages from './messages'

export const i18n = createI18n({
  legacy: false,
  locale: readInitialLocale(),
  fallbackLocale: 'en',
  messages,
})

// 非组件入口与 Vue 组件共享同一 Composer，调用时读取当前语言
export const t: typeof i18n.global.t = i18n.global.t
export const { locale } = i18n.global
