import type { TableSort } from '@codex-proxy/ui'
import type { Account } from '@/api'

import { computed, onMounted, shallowRef, watch } from 'vue'
import { getAccounts } from '@/api'
import { usePagedQuery } from '@/composables/usePagedQuery'

export function useAccountsQuery() {
  const searchQuery = shallowRef('')
  const supplierQuery = shallowRef('')
  const supplierOptions = shallowRef<Array<{ label: string, value: string }>>([
    { label: '全部供应商', value: '' },
  ])
  const planTypeQuery = shallowRef('')
  const providerQuery = shallowRef('')
  const statusQuery = shallowRef('')
  const groupQuery = shallowRef('')
  const sort = shallowRef<TableSort>()
  const accountSummary = shallowRef({
    total: 0,
    normal: 0,
    quotaExhausted: 0,
    rateLimited: 0,
    disabled: 0,
    error: 0,
  })

  const query = usePagedQuery({
    initialPageSize: 20,
    load: ({ page, pageSize }, options) =>
      getAccounts({
        page,
        pageSize,
        search: searchQuery.value,
        supplier: supplierQuery.value.trim() || undefined,
        planType: planTypeQuery.value || undefined,
        provider: providerQuery.value || undefined,
        status: statusQuery.value || undefined,
        groupId: groupQuery.value || undefined,
        sortBy: sort.value?.key,
        sortDirection: sort.value?.direction,
      }, options),
    onSuccess: (result) => {
      accountSummary.value = result.summary
    },
  })

  const accountPagination = computed(() => ({
    currentPage: query.page.value,
    pageSize: query.pageSize.value,
    total: query.total.value,
  }))

  function handlePageChange(page: number) {
    void query.execute({ page })
  }

  function handlePageSizeChange(pageSize: number) {
    void query.execute({ page: 1, pageSize })
  }

  function handleSortChange(nextSort: TableSort | undefined) {
    sort.value = nextSort
    void query.reloadFromStart()
  }

  async function loadSupplierOptions() {
    try {
      const firstPage = await getAccounts({ page: 1, pageSize: 200 })
      const pages = await Promise.all(
        Array.from({ length: Math.max(0, firstPage.page.totalPages - 1) }, (_, index) => getAccounts({ page: index + 2, pageSize: 200 })),
      )
      const values = new Set(
        [firstPage, ...pages]
          .flatMap(result => result.items)
          .map(account => account.supplier?.trim())
          .filter((supplier): supplier is string => Boolean(supplier)),
      )
      supplierOptions.value = [
        { label: '全部供应商', value: '' },
        ...Array.from(values).sort((left, right) => left.localeCompare(right, 'zh-CN')).map(value => ({ label: value, value })),
      ]
    }
    catch {
      // 供应商目录加载失败不影响账号列表和已有文本查询能力。
    }
  }

  async function replaceAccount(updated: Account) {
    // 先取消旧查询并应用接口返回的账号，避免旧响应覆盖最新行数据。
    query.invalidate()
    query.items.value = query.items.value.map(account => account.id === updated.id ? updated : account)

    // 筛选、排序、概览和末页回退仍由回读校准，但不触发整表加载。
    if (!await query.execute({ background: true }))
      return true // 回读失败或被新查询取代时，不依据旧页面取消选择。
    return query.items.value.some(account => account.id === updated.id)
  }

  watch([searchQuery, supplierQuery], (_value, _previous, onCleanup) => {
    const timer = setTimeout(() => {
      void query.reloadFromStart()
    }, 250)
    onCleanup(() => clearTimeout(timer))
  })

  watch([providerQuery, statusQuery, groupQuery, planTypeQuery], () => {
    void query.reloadFromStart()
  })

  onMounted(() => {
    void query.execute()
    void loadSupplierOptions()
  })

  return {
    page: query.page,
    pageSize: query.pageSize,
    totalAccounts: query.total,
    loading: query.loading,
    accounts: query.items,
    loadAccounts: query.execute,
    refreshAccountsSilently: () => query.execute({ silent: true }),
    searchQuery,
    supplierQuery,
    supplierOptions,
    loadSupplierOptions,
    planTypeQuery,
    providerQuery,
    statusQuery,
    groupQuery,
    sort,
    accountSummary,
    accountPagination,
    replaceAccount,
    handlePageChange,
    handlePageSizeChange,
    handleSortChange,
  }
}
