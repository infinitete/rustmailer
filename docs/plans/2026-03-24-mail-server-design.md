# Mail Server Design

**Date:** 2026-03-24

## Goal

基于当前仓库构建一个可通过 Web 管理的自研邮件服务器，首期支持管理员管理域名与邮箱账号，支持 SMTP 收信与提交、IMAP 收信访问，并通过 `certbot` 为 Web、SMTP、IMAP 统一提供自动续签证书。

## Confirmed Scope

- 自研协议服务，不依赖 Postfix、Dovecot 这类成熟邮件服务作为主实现。
- 协议范围为 `SMTP + IMAP + Web Admin`，不包含普通用户 Webmail。
- 邮件与元数据采用全数据库存储。
- 部署方式为 `Docker Compose`。
- `certbot` 统一为 Web 管理后台、SMTP、IMAP 提供 TLS 证书。

## Current State

- `server/` 仍是空白 Rust crate，仅有 `Hello, world!`。
- `frontend/` 已接入 React、Router、Redux 与基础样式能力，但没有业务页面。
- 仓库当前没有数据库层、API 层、协议实现或部署编排。

## Constraints

- 第一阶段必须优先跑通“管理后台配置 + SMTP 入库 + IMAP 读取 + 统一 TLS”闭环。
- 不在首期实现完整互联网邮件生态能力，例如垃圾邮件治理、SPF/DKIM/DMARC、外发重试队列。
- 首期保持单机 `docker-compose` 可部署，不提前做分布式复杂拆分。
- 代码结构要支持后续把协议适配层拆分为独立进程，但第一期不强行拆服务。

## Options Considered

### Option 1: 单体多协议服务

由一个 Rust 进程同时提供 HTTP、SMTP、IMAP 服务，内部按模块划分。

**Pros**
- 启动快，部署简单。
- 配置、数据库连接、TLS 状态共享容易。

**Cons**
- 协议实现与业务逻辑容易耦合。
- 后续拆服务时需要再做边界梳理。

### Option 2: 管理 API + 独立协议工作进程

HTTP、SMTP、IMAP 分成独立服务，共享数据库。

**Pros**
- 进程边界清晰。
- 协议层可以独立扩容和重启。

**Cons**
- 以当前仓库基础看，第一期工程复杂度偏高。
- 需要更早引入配置同步、跨服务协调和更多运维编排。

### Option 3: 事件驱动核心 + 协议适配层

把系统拆成核心邮件引擎和多个协议适配层。协议只做翻译，核心统一处理认证、落库、状态变更和事件流。

**Pros**
- 长期演进空间最大。
- 业务规则不会分散在 SMTP、IMAP、HTTP 三套实现里。
- 后续拆服务、接消息总线或增加新协议成本较低。

**Cons**
- 第一阶段设计与实现成本更高。
- 需要先定义清晰的命令、查询和领域事件边界。

## Recommended Approach

采用 **Option 3**：实现一个单进程部署的“事件驱动核心 + 协议适配层”系统。

部署层面仍维持一个 `server` 容器，以控制第一阶段复杂度；代码层面则显式拆分为：

- `mail-core`
- `smtp-adapter`
- `imap-adapter`
- `http-admin-adapter`
- `tls-adapter`
- `storage`

这样可以兼顾第一阶段落地效率与后续演进能力。

## Architecture

### Core engine

`mail-core` 是唯一的业务核心，负责：

- 域名与邮箱账号管理
- 管理员认证
- 邮件接收与持久化
- 文件夹与 IMAP 状态维护
- 统一错误类型
- 领域事件发布

核心通过命令与查询接口向外提供能力，例如：

- `CreateDomain`
- `CreateMailbox`
- `AuthenticateMailbox`
- `ReceiveInboundMessage`
- `ListFolders`
- `FetchMessages`
- `UpdateMessageFlags`

### Protocol adapters

- `http-admin-adapter`
  - 提供管理员登录、域名管理、邮箱账号管理、系统状态与证书状态 API。
- `smtp-adapter`
  - 处理 `EHLO/HELO`、`STARTTLS`、`AUTH`、`MAIL FROM`、`RCPT TO`、`DATA`。
  - 将会话翻译为核心命令。
- `imap-adapter`
  - 处理 `LOGIN`、`LIST`、`SELECT`、`FETCH`、`STORE`、`SEARCH`、`EXPUNGE` 和基础 `UID` 命令。
  - 将协议命令翻译为核心查询与命令。
- `tls-adapter`
  - 负责读取 `certbot` 生成的证书，并为 HTTP、SMTP、IMAP 提供热重载 TLS 配置。

### Event flow

第一阶段不引入 Kafka 或 NATS 等外部总线，采用单进程内事件分发：

- `MessageAccepted`
- `MessageStored`
- `MailboxProvisioned`
- `CertificateReloaded`

这样可以先明确领域边界，后续需要异步扩展时再替换为外部总线。

## Data Model

首期采用 PostgreSQL，建议实体如下：

- `admins`
  - 管理员账号、密码哈希、角色、最后登录时间
- `domains`
  - 托管域名、启用状态、证书状态、DNS 期望值
- `mailboxes`
  - 邮箱地址、归属域名、密码哈希、启用状态、配额
- `mail_folders`
  - 每个邮箱的逻辑文件夹，至少包含 `INBOX`、`Sent`、`Drafts`、`Trash`
- `messages`
  - 原始 RFC822 内容、摘要头、大小、接收时间
- `message_delivery`
  - 邮件与邮箱/文件夹的映射，支持一封消息关联多个收件人
- `message_flags`
  - IMAP flags，例如 `Seen`、`Answered`、`Deleted`、`Flagged`
- `imap_folder_state`
  - `UIDVALIDITY`、`UIDNEXT`
- `imap_message_uids`
  - 消息在特定文件夹内的 UID 映射
- `audit_logs`
  - 管理员操作、协议错误摘要、证书热加载结果

设计原则：

- 邮件正文只存一份，通过投递映射复用。
- IMAP 相关状态单独建模，不依赖动态计算。
- 管理后台统计、状态页和协议读取共用同一份事实数据。

## Protocol Boundaries

### SMTP phase 1

支持：

- `EHLO/HELO`
- `STARTTLS`
- `AUTH LOGIN`
- `AUTH PLAIN`
- `MAIL FROM`
- `RCPT TO`
- `DATA`
- 本地域名收件
- 已认证邮箱账号提交邮件

暂不支持：

- 互联网外发重试队列
- SPF / DKIM / DMARC 完整验证
- 反垃圾与灰名单

### IMAP phase 1

支持：

- `LOGIN`
- `LIST`
- `SELECT`
- `FETCH`
- `STORE`
- `SEARCH`
- `EXPUNGE`
- 基础 `UID` 命令

暂不支持：

- `IDLE`
- `ACL`
- `QUOTA`
- `MOVE`
- `CONDSTORE`
- `QRESYNC`

### HTTP admin phase 1

支持：

- 管理员登录
- 域名 CRUD
- 邮箱账号 CRUD
- 服务健康状态
- 证书状态
- 基础审计日志查看

暂不支持：

- 普通用户邮箱访问
- Webmail
- 多租户复杂权限模型

## Certbot And Docker Compose

建议 `docker-compose` 包含以下服务：

- `postgres`
- `server`
- `frontend`
- `nginx`
- `certbot`

### Port layout

- `80` -> `nginx`
- `443` -> `nginx`
- `25` -> `server`
- `587` -> `server`
- `143` -> `server`
- `993` -> `server`

### Certificate flow

- `nginx` 暴露 `/.well-known/acme-challenge/`
- `certbot` 使用 `webroot` 模式签发证书
- 证书目录以共享卷方式挂载到 `server`
- `server` 通过文件监听或周期检查热加载新证书
- 管理后台展示当前证书到期时间、主题名与最近热加载状态

首期建议使用一张 SAN 证书覆盖：

- `admin.example.com`
- `mail.example.com`

## Error Handling

核心定义统一错误类型，例如：

- `AuthError`
- `DomainNotFound`
- `MailboxDisabled`
- `TlsUnavailable`
- `StorageConflict`
- `ProtocolViolation`

适配层负责协议翻译：

- HTTP -> JSON 错误与状态码
- SMTP -> 4xx/5xx 状态码
- IMAP -> `NO` / `BAD`

证书热加载失败时保持旧证书继续服务，不因新证书加载失败导致服务中断。

## Testing And Verification

首期测试策略：

- Rust 单元测试
  - 认证
  - 域名匹配
  - 文件夹初始化
  - IMAP UID 分配
  - 消息落库与 flags 更新
- Rust 集成测试
  - Admin API 最小流程
  - SMTP 最小收件流程
  - IMAP 最小读取流程
- 前端验证
  - `npm run lint`
  - `npm run build`
- 全局验证
  - `cargo test`
  - `cargo fmt --check`

## Phase 1 Deliverables

第一阶段交付内容：

- 管理员后台
  - 登录
  - 域名管理
  - 邮箱账号管理
  - 服务状态页
  - 证书状态页
- SMTP
  - TLS
  - 基础认证提交
  - 本地域名收件入库
- IMAP
  - TLS
  - 登录
  - 列目录、读取邮件、更新 flags
- `docker-compose`
  - PostgreSQL
  - Nginx
  - Certbot
  - Frontend
  - Server
- 统一证书热加载

明确不在第一阶段内的内容：

- 完整互联网外发投递
- 反垃圾、反病毒和策略引擎
- Webmail
- 每个托管域名独立自动签发证书
- 多节点部署
