# v3.52.0

## 新增功能

- 请求记录可按历史账号名称、邮箱或账号 ID 筛选
- 内置 GPT-6.2 Astra 和 Sol 价目，Ultrafast 按常规价估算

## 优化改进

- 请求记录用双闪电标识 Ultrafast，用单闪电标识 Fast
- 请求错误来源图标改为无底色显示

## 升级须知

- `GET /api/admin/usage/records` 使用 `accountSearch`，继续接受旧参数 `accountName` 作为别名
- 自行部署时请确认 PostgreSQL 版本为 16 或更高，默认 Compose 使用 PostgreSQL 18
