import { useI18n } from 'vue-i18n'
import { isLocale, localeOptions } from '@/i18n/locale'

export function useLocale() {
  const { locale } = useI18n({ useScope: 'global' })

  function setLocale(value: unknown) {
    if (isLocale(value))
      locale.value = value
  }

  return { locale, localeOptions, setLocale }
}
