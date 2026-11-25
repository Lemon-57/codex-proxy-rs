import type { ApiKeyAccountForm } from './connection'
import { t } from '@/i18n'
import { isRecord } from '@/utils/data'

export function apiKeyDocument(form: ApiKeyAccountForm) {
  return {
    provider: 'openai',
    authentication_kind: 'api_key',
    base_url: form.base_url.trim(),
    api_key: form.apiKey,
    protocol: form.protocol,
    transport: form.transport,
  }
}

export function tokenDocuments(mode: 'access_token' | 'refresh_token', text: string, expectedEmail = '') {
  const tokens = text.split(/\r?\n/).map(token => token.trim()).filter(Boolean)
  const access = mode === 'access_token'
  if (!tokens.length) {
    throw new Error(t('accounts.import.pasteAtLeastOne', {
      label: access ? 'Access Token' : 'Refresh Token',
    }))
  }
  if (access) {
    return tokens.map(token => ({
      accounts: [
        { accessToken: token },
      ],
    }))
  }
  const seenEmails = new Set<string>()
  const seenTokens = new Set<string>()
  return tokens.map((line, index) => {
    // RT 不携带可读身份，先提交预期邮箱供保护层去重并选择固定代理
    const delimiter = line.indexOf('----')
    const separator = delimiter >= 0 && line.slice(0, delimiter).includes('@')
      ? delimiter
      : -1
    const email = (separator >= 0
      ? line.slice(0, separator)
      : tokens.length === 1
        ? expectedEmail
        : '').trim().toLowerCase()
    const token = separator >= 0
      ? line.slice(separator + 4).trim()
      : line
    if (!email || email.length > 254 || !/^[^\s@]+@[^\s@]+$/.test(email) || !token || /\s/.test(token))
      throw new Error(t('accounts.import.refreshTokenIdentityRequired', { line: index + 1 }))
    if (seenEmails.has(email) || seenTokens.has(token))
      throw new Error(t('accounts.import.duplicateRefreshTokenAccount', { line: index + 1 }))
    seenEmails.add(email)
    seenTokens.add(token)
    return {
      accounts: [
        { email, refreshToken: token },
      ],
    }
  })
}

export function splitDocument(document: Record<string, unknown>): Record<string, unknown>[] {
  const data = isRecord(document.data) && Array.isArray(document.data.accounts)
    ? document.data
    : document
  if (!Array.isArray(data.accounts) || !data.accounts.length)
    return [document]
  return data.accounts.map((account) => {
    const item: Record<string, unknown> = {
      ...data,
      accounts: [account],
    }
    if (Array.isArray(data.proxies)) {
      // 保留同键的全部记录，让 Provider 继续拒绝重复引用，不复制其他账号的代理
      const key = isRecord(account)
        ? account.proxy_key
        : undefined
      item.proxies = data.proxies.filter(proxy => typeof key === 'string' && isRecord(proxy) && proxy.proxy_key === key)
    }
    return data === document
      ? item
      : { ...document, data: item }
  })
}
