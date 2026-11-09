# zhex 交付与验收报告

> 第一轮交付的历史记录。第二轮加速与已完成的 crates.io 0.1.0 真实发布，见
> [最新发布报告](RELEASE_REPORT.md)；下文保留当时的结果与发布状态。

zhex 0.1.0 已实现并分阶段提交到
[LIghtJUNction/zhex](https://github.com/LIghtJUNction/zhex)。核心功能、安全边界、
独立测试、基准工具、CI 和自动发布工作流已完成。**相对每组最强基线争取 2 倍加速的
性能目标未达标。** 本次没有真实发布到 crates.io，也没有创建 GitHub Release。

## 实现范围

发布库始终声明 `#![no_std]`，只使用 `core`，普通、构建、开发及 target-specific
第三方依赖均为零，没有 `build.rs`、`alloc`、C/FFI 或外部运行时。调用方提供输出
切片，核心 API 不分配堆内存。竞品依赖只位于独立的 `comparison/` 工作区，发布包
内容检查已确认它们没有进入正式 crate。

| API | 行为 |
| --- | --- |
| `encode_lower` | 写出小写 ASCII 十六进制，返回写入长度 |
| `encode_upper` | 写出大写 ASCII 十六进制，返回写入长度 |
| `decode` | 接受大小写混合的 ASCII 十六进制，返回写入长度 |
| `encoded_len` | 检查长度倍增是否溢出 |
| `decoded_len` | 拒绝奇数文本长度，计算解码后的字节数 |

```rust
let bytes = [0x00, 0xab, 0xff];
let mut text = [0; 6];
assert_eq!(zhex::encode_lower(&bytes, &mut text), Ok(6));
assert_eq!(&text, b"00abff");

let mut output = [0; 3];
assert_eq!(zhex::decode(b"00aBfF", &mut output), Ok(3));
assert_eq!(output, bytes);
```

编码将每个字节分成高 4 位和低 4 位，各自映射成一个字符。解码完成相反的组合，
同时检查字符是否合法。SIMD 对一组字节同时执行这些操作；短输入和不适用 SIMD
的目标使用标量路径。x86 的 AVX2 分派通过 CPUID、OSXSAVE 和 XGETBV 检查 CPU
与操作系统状态，AArch64 只在编译目标保证 NEON 时启用相应后端。

空输入成功。解码拒绝空白、`0x` 前缀、奇数长度和非 ASCII 十六进制字节。
错误优先级为奇数长度、容量不足、首个非法字节；编码先检查长度溢出再检查容量。
`InvalidByte { index, byte }` 使用原始输入中的字节位置。长度和容量错误保持输出
完全不变；非法字符可能留下已修改的输出前缀，修改长度不作保证。实现没有为了
事务性输出增加全量字符预扫描，也不依赖切片尾部 padding。

## 性能结果

主要实验使用 Intel Xeon Platinum 8573C、Linux x86-64、Rust 1.99.1，在共享云环境
固定一个逻辑 CPU 上运行。预先固定 8 种主要长度、3 种有效转换和 4 组输入/输出
偏移，共 96 个主要场景。每个场景与该场景中最快的合格竞品比较，随后计算速度比
的几何平均值。速度比 1 表示持平，2 表示达到两倍速度。

| 编译配置 | 相对每组最强基线的几何平均速度比 | 达到 2 倍的主场景 |
| --- | ---: | ---: |
| generic | 0.744× | 0 / 96 |
| `-C target-cpu=native` | 0.723× | 0 / 96 |

这些结果显示，本次实现仍落后于每组最强基线。共享环境的抖动限制了细微差异的
解释范围；这里没有把 generic/native 的小幅差异解释成稳定的性能退化，也没有
以未经测量的内存带宽上限解释目标未达标。

对照版本为 `hex 0.4.3`、`faster-hex 1.0.0`、`const-hex 1.19.3`、`hex-simd 0.8.0`，
以及额外检查的 `better-hex 1.0.1`、`fashex 0.1.1`、`hex-turbo 0.4.0`。比较统一使用
带校验、输出预分配的切片接口。`hex` 缺少对应的大写无分配接口，其相关样本明确
记录为 unsupported。需要补充首个错误位置的适配器只在解码已经失败后扫描。

完整覆盖包括 0–64 字节、向量边界、数百字节、4 KiB、64 KiB、1 MiB、8 MiB 和
不同对齐偏移。每个完整竞品配置保留 58,184 条实际计时样本。短输入以 ns/op
报告，大输入以 payload GiB/s 报告：编码按输入字节数，解码按输出字节数计量。
生成数据、分配、预热、结果验证和记录均在对应测量边界之外。

标量算术编码、查表解码、手写 SIMD、运行时分派和短输入路径均有独立实验。
没有整体收益的 64 字符解码方案已舍弃，原始样本和源码快照仍保留。
详见[完整性能报告](https://github.com/LIghtJUNction/zhex/blob/main/comparison/PERFORMANCE.md)、
[固定实验协议](https://github.com/LIghtJUNction/zhex/blob/main/comparison/PLAN.md)和
[复现说明](https://github.com/LIghtJUNction/zhex/blob/main/comparison/README.md)。

## 实际验证

核心代码提交
[`60d913d`](https://github.com/LIghtJUNction/zhex/commit/60d913d2501a858825cf774ac454efe25e509234)
对应的 [CI 运行 42648275776](https://github.com/LIghtJUNction/zhex/actions/runs/42648275776)
已 **11 / 11 作业通过**。

| 验证内容 | 结果 |
| --- | --- |
| x86-64 stable / Rust 1.70.0 | debug、release、文档测试、标量、native、强制 AVX2 均通过 |
| 实际 AArch64 stable / Rust 1.70.0 | ARM runner 上运行通过，包括强制 NEON |
| 裸机 `thumbv7em-none-eabi` | stable 和 MSRV 编译通过 |
| Miri | 标量测试及文档测试通过，解释执行时缩减穷举循环规模 |
| 格式与 Clippy | 通过，警告按错误处理 |
| 零依赖与发布包边界 | 通过 |
| `cargo package --list --locked` | 通过 |
| `cargo package --locked` | 通过 |
| `cargo publish --dry-run --locked` | 通过，未上传 |
| x86 / ARM benchmark smoke | 竞品适配器和标量/SIMD 内核均实际运行通过 |

正确性测试使用独立参考实现，覆盖全部单字节、全部双字节组合、非法字节及其
位置、混合大小写、错误优先级、输出哨兵、容量少一字节、长度溢出、切片偏移、
向量边界及大输入。另有并发首次调用测试和 CPU/OS 支持缺失时的检测顺序测试。
测试结果、日志摘录、原始 runner CSV 和 artifact 校验信息见
[验证索引](https://github.com/LIghtJUNction/zhex/blob/main/validation/README.md)。

## 发布状态与交付位置

Release 工作流支持推送 `vX.Y.Z` 标签和手动选择已存在的标签，校验标签版本，
在标签对应的确定 commit 上完成检查。只有真正执行 `cargo publish --locked`
的步骤会获得 `CARGO_REGISTRY_TOKEN`；缺少 Secret 时明确退出。默认权限为读取，
只有后续创建 GitHub Release 的作业拥有写权限；发布串行执行且不中途取消。

当前状态为 **自动发布流程已配置、dry-run 已通过，尚未真实发布**。正式发布成功
后工作流才创建 GitHub Release。重复版本不会覆盖已有 crates.io 版本。

分阶段提交为：

1. [`183ff65`](https://github.com/LIghtJUNction/zhex/commit/183ff656b2aded4980a3b93e1cbd510693285238)：标量正确实现与独立穷举测试。
2. [`61c58f2`](https://github.com/LIghtJUNction/zhex/commit/61c58f2cd68b5d3cbda4aecdd4ba2c8b6f5ac30b)：实测标量优化、AVX2/NEON 和独立基准工程。
3. [`b236051`](https://github.com/LIghtJUNction/zhex/commit/b236051dc70bcbfffd177ef5fcd3fb6a58e3e23a)：CI、打包边界和自动发布。
4. [`60d913d`](https://github.com/LIghtJUNction/zhex/commit/60d913d2501a858825cf774ac454efe25e509234)：实测 AVX2 解码精简及额外内核 smoke。

正式源代码、报告及原始结果位于 GitHub 仓库。
