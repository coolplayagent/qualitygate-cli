# 自检与 fixtures

[English](../../en/04-contributor-guide/02-selfcheck-and-fixtures.md) · [本卷目录](README.md)

`qualitygate selfcheck` 是随 CLI 发布的有界回归 harness。它物化精选仓库，执行生产规则、计划和报告
路径，再将归一化结果与独立编写的 golden 文件比较。

```bash
qualitygate selfcheck
qualitygate selfcheck --fixture minimal --rule commit-message
```

语料覆盖通过、违规、证据缺失、非法输入、超时、策略演进、自定义契约、签名记录、报告适配器和兼容性。
过滤器适合定位问题，但实现变化后不能替代完整语料运行。

每次结果报告 verification conclusion、已验证形态、已知限制和未验证假设。一致只表示在这些 fixture
形态下未发现差异，不能证明所有仓库、工具版本、平台或外部信任部署都正常。

fixture 必须确定、受限且不依赖用户状态。Git fixture 只在临时仓库中设置本地身份。网络和真实工具
producer 属于单独显式运行的测试。golden 更新与行为变化采用相同审核，不能为消除回归而直接重生成
预期输出。

修改 CLI 或 Skill 包后，先用匹配的发布 runtime 运行完整 selfcheck，再运行仓库质量门禁。golden
不匹配与执行缺口必须分别保留。安装本地构建的开发 runtime 是独立显式操作，不能描述为公开发布版。
