# v3.30.2

## 升级说明

- 前后端需同步更新，管理端使用新增的账号指标原始值、相对时间结构和平均延迟数值生成本地化显示
- Dashboard 趋势 `points[]` 移除页面未使用的 `latency`、`maxLatency`、`maxLatencyValue`、`minLatency`、`minLatencyValue`、`firstTokenP50Ms`、`admissionDecisionP95Ms`、`accountSelectionWaitP95Ms` 和 `capacityUtilization`；自行对接这些字段的客户端需同步适配，平均延迟仍使用 `latencyValue`，内部调度分位数和容量分布通过用量洞察查询

## 修复与调整

- 修复 Dashboard 活跃账号的指标名称、数字和最近使用时间，以及账号额度刷新时间未随界面语言切换的问题
- 相对时间继续由服务端按查询锚点、当前时间和部署时区确定，前端只翻译分类与单位，保留分钟、小时、天的边界及未来时间的完整日期显示
- 调整分析统计页平均延迟的数值与单位间距，统一账号和 API Key 编辑弹窗的保存文案，并让账号、分组、密钥、代理及主题设置页复用页面标题的响应式高度

## 性能优化

- 统计直方图先合并相同的正值样本再计算对数分桶，减少重复计算，保留原有桶边界、样本权重和统计结果；实际收益取决于样本重复度
- 精简 Dashboard 趋势响应中未使用的字段，减少传输体积
