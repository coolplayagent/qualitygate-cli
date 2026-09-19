# 规则配置

[English](../../en/01-user-guide/03-rule-configuration.md) · [本卷目录](README.md)

先使用只读发现命令：

```bash
qualitygate --root . rules categories --format json
qualitygate --root . rules list --source all --format json
qualitygate --root . rules describe RULE_ID --format json
qualitygate --root . config --show --format json
```

目录同时展示已启用规则、未选择的内置包和项目规则。`enabled`、`source`、`language`、能力需求、
参数、严重级别与检测限制是不同字段。查看或分配类别不会启用检查，也不代表策略已获批准。

候选变更必须显式执行：

```bash
qualitygate --root . rules enable RULE_ID
qualitygate --root . rules disable RULE_ID
qualitygate --root . rules configure RULE_ID --set key=value
```

写入前会验证完整候选配置，并采用原子替换。参数无效、ID 重复、能力未知或规则包不可读都会失败。
语法有效的候选仍需经过团队正常审核和采用流程。

项目规则默认位于 `qualitygate/rules`。应以导出的 Schema 编写、验证候选后再发布：

```bash
qualitygate rules schema
qualitygate rules validate candidate.yaml
qualitygate rules generate --input reviewed-source.md
```

生成过程保留来源摘要和能力边界，但不能替代规范审核。模式规则只是有界信号，例如疑似凭据文本或
不安全调用并不能证明漏洞。必需事实缺失、过期、格式错误或超出适配器能力时，结果必须保持未完成。

文件存在性与大小要求使用 file contract；外部分析器债务使用 report ratchet。二者都不是临时编造的
内置规则 ID。
