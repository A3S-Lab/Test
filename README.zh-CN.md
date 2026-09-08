<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test connects a selected rendered element to its current page revision, owning source, and fresh verification evidence">
</p>

<p align="center">
  <strong>Language / 语言:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=1264ff&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <a href="https://www.npmjs.com/package/@a3s-lab/testkit"><img src="https://img.shields.io/npm/v/@a3s-lab/testkit?style=flat-square&color=1264ff&label=testkit" alt="Test Kit npm version"></a>
  <a href="https://a3s-lab.github.io/Test/"><img src="https://img.shields.io/badge/docs-%E4%B8%AD%E6%96%87%20%7C%20English-1264ff?style=flat-square" alt="Chinese and English documentation"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-56657b?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-56657b?style=flat-square" alt="MIT License"></a>
</p>

<h3 align="center">查看实际渲染的内容。找到拥有它的来源。证明改变。</h3>

<p align="center">
  A3S Test 为编码代理提供了值得信赖的接口反馈循环：<br>
  新鲜的浏览器事实、修订绑定的操作、源感知审查和可检查的证据。
</p>

<p align="center">
  <a href="https://a3s-lab.github.io/Test/"><strong>中文文档</strong></a> ·
  <a href="https://a3s-lab.github.io/Test/en/"><strong>英文文档</strong></a> ·
  <a href="#install-only-what-you-need">安装</a> ·
  <a href="#run-one-real-browser-loop">快速入门</a> ·
  <a href="#add-the-page-context-test-kit">测试套件</a> ·
  <a href="#architecture">架构</a>
</p>

## 产品就是证明

<p align="center">
  <img src="./assets/readme/testkit-review.png" width="100%" alt="The real A3S Test documentation experience with a rendered checkout page and the Test Kit review panel open on the right">
</p>

<p align="center"><sub>在文档站点内运行的真实测试套件包。这个公开演示将结果保留在当前选项卡中；它不会连接到修复代理或编辑源。</sub></p>

编码代理永远不应该修复它仅仅想象的接口。 A3S测试保持
从页面到回归的路径简短且可检查：

|                  |问题 | A3S测试答案|
| ---------------- | -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| **01 · 观察** |现在页面呈现什么？        |浏览器语义加上与当前页面修订版相关的有界测试套件上下文
| **02 · 定位** |哪个来源拥有可见结果？ |稳定的候选定位器、组件所有权、几何形状和排名源跨度 |
| **03·证明** |更改是否有效且没有漂移？    |来自较新版本、仅附加会话记录和确定性 ACL 覆盖范围的证据 |

浏览器事实、模型建议、人工授权和工作空间突变仍然存在
单独的当局。源跨度可以解释在哪里查找；它从不给予
编辑权限。

## 选择最短的入口点

|你需要…… |从…开始你保留... |
| ---------------------------------------------------------------- | ----------------- | -------------------------------------------------------------------------------------------------- |
|探索不熟悉的流程或重现错误 | CLI + 代理技巧 |观察结果、承认的行动、屏幕截图、事件和终端报告 |
|指向呈现的问题并将其返回到代码 |网络测试套件|当前修订、元素或区域、组件、候选源和人类意图 |
|重复其操作和断言已知的路径 | ACL 套件 |具有现有证据的确定性局部或 CI 回归 |

对于普通浏览器自动化来说，测试套件是可选的。当组件时添加
所有权、源映射、渲染几何、视觉参考或人工
标记大大改善了任务。

## 只安装你需要的

### CLI 和代理技能

发布安装程序下载平台存档，验证其 SHA-256，
并为检测到的编码代理安装匹配的便携式 A3S 测试技能。

Mac OS 或 Linux：

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh
```

Windows PowerShell：

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))
```

当可重复性很重要时，固定当前的稳定版本：

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --version v1.0.1
```

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Version v1.0.1
```

### 网络测试套件

从 npm 安装仅用于开发的 React 集成：

```bash
npm install --save-dev @a3s-lab/testkit@0.6.2
npm ls @a3s-lab/testkit
```

`@a3s-lab/testkit` 0.6.2 发布在官方 npm 注册表上
GitHub OIDC 出处。其版本的推进独立于 CLI。

【打开完整安装指南](https://a3s-lab.github.io/Test/guide/installation.html)

## 运行一个真正的浏览器循环

使用页面可以证明的结果启动持久会话：

```bash
a3s-test agent start http://127.0.0.1:3000/checkout \
  --session checkout \
  --goal "Complete checkout with the fixture account" \
  --success "The confirmation heading is visible" \
  --json

a3s-test agent observe --session checkout --interactive --json
```

观察返回绑定到一个观察的新语义引用：

```text
observation_id: 1
@e1 [button] Continue
```

承认一项反对该观察的行动，保留最少的证据，并且
明确完成：

```bash
a3s-test agent click @e1 \
  --session checkout \
  --observation 1 \
  --json

a3s-test agent screenshot screenshots/confirmation.png \
  --session checkout \
  --json

a3s-test agent finish \
  --session checkout \
  --status passed \
  --summary "Checkout completed and confirmation was observed" \
  --json
```

浏览器可以关闭；结果仍然可以检查：

```text
.a3s-test/agent-sessions/checkout/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
    └── screenshots/confirmation.png
```

[继续进行第一个Web测试](https://a3s-lab.github.io/Test/guide/)

## 添加页面上下文测试套件

在应用程序根目录安装两个组件并明确禁用它们
生产中：

```tsx
import { A3SReviewOverlay, A3STestKit } from "@a3s-lab/testkit/react";

const testKitEnabled = import.meta.env.DEV;

<A3STestKit enabled={testKitEnabled} page={{ id: "app" }}>
  <App />
  <A3SReviewOverlay enabled={testKitEnabled} locale="auto" />
</A3STestKit>;
```

对于默认审阅路径来说这已经足够了：

1. 打开右侧的审阅面板。
2. 选择一个元素或在某个区域上拖动。
3. 描述预期结果并慎重提交。

文本、多选、绘图和布局位于**更多工具**下。设计
板从右侧滑出，可以容纳草图或浏览器页面的一部分
内容无需请求全屏共享权限。无头者
上下文运行时也可以在没有可见的审阅覆盖层的情况下运行。

本地项目循环在之后验证实时`a3s.test.testkit-handshake/1`
在报告准备就绪之前进行水合作用。其 `init`、`doctor` 和 `dev` 命令为
包含在 v1.0.1 版本中。

仅在有帮助的地方添加显式源边界：

```tsx
import { A3STestBoundary } from "@a3s-lab/testkit/react";

<A3STestBoundary
  id="checkout-form"
  name="Checkout form"
  source={{ file: "src/Checkout.tsx" }}
>
  <Checkout />
</A3STestBoundary>;
```

Framework adapters may register an exact DOM owner and an explicitly supplied
Source Map v3. A resulting source-mapping record keeps ranked spans,
confidence, origin, and exact or ancestor relation. It is navigation evidence,
not source-edit authority.

[阅读测试套件集成指南](https://a3s-lab.github.io/Test/guide/testkit.html)

## 是什么让循环值得信赖

### 行动前的新鲜感

每个可操作的参考都属于一个观察或页面修订。页面变更
使浏览器语义、几何图形、屏幕截图和任何测试套件定位器过期
精确的增量无法证明不变。

### 执行前键入控制

动作是封闭的变体。架构、目标类型、驱动程序能力、来源
在输入到达 Web、GUI 或 TUI 之前验证策略和会话状态。
不受支持的行为无法关闭而不是被近似。

### 突变前的人类权威

选择、绘制草图、捕获或保存草稿并不授权来源
改变。调查结果只能通过明确的方式进入维修分类账
提交；工作区突变属于单独授权的编码
代理。

### 成功之前的新证据

修复必须在较新的渲染版本中证明其成功。 A3S测试可以
从可信配置中选择重点项目检查，扩展到更广泛的范围
当观察到的影响需要时回归，并保留最小的已证明的
浏览器路径作为 ACL。

【阅读权威及安全模型](https://a3s-lab.github.io/Test/concepts/authority-and-safety.html)

## 保留经过验证的路径作为 ACL

代理会话发现未知路径。 ACL 仅重复其操作的路径，
等待，成功条件是明确的：

```acl
suite "product-smoke" {
    version = 1

    scenario "home-page" {
        name = "Open the home page"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {
            url = "https://example.com"
        }

        wait "loaded" {
            load = "networkidle"
        }

        expect "heading" {
            text = "Example Domain"
            stable_for_ms = 300
            sample_interval_ms = 50
        }

        screenshot "evidence" {
            path = "home.png"
        }
    }
}
```

```bash
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

普通操作永远不会隐式重试。仅显式采样，
只读断言在其有限的稳定性窗口内重复。

## 架构

<p align="center">
  <img src="./assets/readme/architecture.svg" width="100%" alt="Agent exploration and deterministic ACL regression enter one typed core, dispatch through owned Web, GUI, or TUI drivers, and retain evidence and cleanup results">
</p>

|层|责任|
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
|浏览器+测试套件|渲染后语义、页面修订、稳定候选定位器、几何、组件所有权、源跨度和可选审查 |
| Rust 核心 + 会话 |类型化操作、观察引用、策略、权限、持久状态、证据、结果和生命周期契约 |
|评论 + Surface 驱动程序 |显式修复提交以及 Web、GUI 和 TUI 感知、调度、流程所有权和清理 |
|验证+ACL |源头限制检查、影响驱动的扩展、新鲜证据和可重复的确定性覆盖 |

每个启动的程序都属于一个拥有的进程树。超时和
取消会收获该树而不关闭不相关的开发人员会话。

## 表面支持

|表面|当前边界|
| -------- | ------------------------------------------------------------------------------------------------ |
|网页 |通过 A3S 浏览器或兼容的独立浏览器实现持久代理会话和 ACL |
|图形用户界面 | macOS CUA 集成在真实的 arm64 主机上经过认证；其他桌面后端仍在审查中
|途易 | ACL 套件通过拥有的 PTY / ConPTY 进程树和有界终端语义 |

所有表面共享核心行动、政策、证据、结果和清理契约；
每个适配器仍然拥有感知和执行。

## 文档

- [运行第一个Web测试](https://a3s-lab.github.io/Test/guide/)
- [仅安装您需要的部件](https://a3s-lab.github.io/Test/guide/installation.html)
- [添加网页测试套件](https://a3s-lab.github.io/Test/guide/testkit.html)
- [了解页面上下文](https://a3s-lab.github.io/Test/concepts/page-context.html)
- [比较探索和ACL](https://a3s-lab.github.io/Test/guide/workflows.html)
- [检查每项能力](https://a3s-lab.github.io/Test/reference/capabilities.html)

存储库级协议和实现参考保留在[`docs/`](docs/)中。

<details>
<summary><strong>开发检查</strong></summary>

从此存储库运行检查，而不是从父 monorepo 运行检查：

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

npm --prefix packages/testkit test
npm --prefix website run check
npm --prefix website run build
npm --prefix website run check:site
```

</details>

## 许可证

A3S Test 和 `@a3s-lab/testkit` 已根据 [MIT 许可证](LICENSE) 获得许可。